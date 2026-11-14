import { t } from './i18n';
import { useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  ArrowRight,
  CheckCheck,
  ChevronDown,
  Database,
  Folder,
  FolderSearch,
  LoaderCircle,
  Search,
  ShieldCheck,
  SquareTerminal,
  X,
} from 'lucide-react';
import { call } from './api';
import { Modal, ProjectGlyph, Tags } from './components';
import Select from './Select';
import {
  typeNames,
  type Candidate,
  type ComponentCandidate,
  type Project,
  type ScanProgress,
  type Settings,
} from './types';

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));
const shortPath = (path: string) => path.replace(/^\/Users\/[^/]+/, '~');
const components = (candidate: Candidate) => candidate.components || [];
const boundaryNames: Record<string, string> = {
  GIT: '仓库项目',
  WORKSPACE: '工作区项目',
  COMBINED: '前后端目录组合',
  MANIFEST: '清单识别',
};
function matches(value: Candidate | ComponentCandidate, terms: string[]) {
  const text = [
    value.name,
    value.canonical_path,
    value.project_type,
    t(typeNames[value.project_type]),
    ...value.frameworks,
    ...value.build_tools,
    ...value.evidence,
  ]
    .join(' ')
    .toLocaleLowerCase();
  return terms.every((term) => text.includes(term));
}
const termsOf = (value: string) => value.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
const matchesProject = (candidate: Candidate, terms: string[]) =>
  matches(candidate, terms) || components(candidate).some((component) => matches(component, terms));

export default function ScanModal({
  settings,
  close,
  imported,
  report,
}: {
  settings: Settings;
  close: () => void;
  imported: (projects: Project[]) => void;
  report: (error: unknown) => void;
}) {
  const [root, setRoot] = useState('');
  const [depth, setDepth] = useState(settings.scan_depth);
  const [limited, setLimited] = useState(false);
  const [query, setQuery] = useState('');
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState('');
  const idRef = useRef<string | null>(null);
  const closingRef = useRef(false);
  const initialized = useRef<string | null>(null);
  const allRef = useRef<HTMLInputElement>(null);
  const scanning = progress?.status === 'scanning';
  const completed = progress?.status === 'completed';
  const candidates = completed ? progress.candidates : [];
  const terms = termsOf(query);
  const visible = candidates.filter((candidate) => matchesProject(candidate, terms));
  const visibleIds = new Set(visible.map((candidate) => candidate.id));
  const eligible = visible.filter((candidate) => !candidate.imported);
  const selectedVisible = eligible.filter((candidate) => selected.has(candidate.id)).length;
  const hiddenSelected = candidates.filter(
    (candidate) => selected.has(candidate.id) && !visibleIds.has(candidate.id),
  ).length;
  const updating = candidates.filter(
    (candidate) => candidate.imported && selected.has(candidate.id),
  ).length;
  const serviceCount = candidates.reduce(
    (count, candidate) =>
      count + components(candidate).filter((component) => component.runnable).length,
    0,
  );

  useEffect(() => {
    if (allRef.current)
      allRef.current.indeterminate = selectedVisible > 0 && selectedVisible < eligible.length;
  }, [selectedVisible, eligible.length, completed]);
  useEffect(() => {
    closingRef.current = false;
    return () => {
      closingRef.current = true;
      if (idRef.current) void call('scan_cancel', { id: idRef.current }).catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (!progress || progress.status !== 'scanning') return;
    let cancelled = false;
    let inFlight = false;
    const timer = setInterval(() => {
      if (inFlight) return;
      inFlight = true;
      call<ScanProgress>('scan_status', { id: progress.id })
        .then((result) => {
          if (cancelled) return;
          setProgress(result);
          if (result.status === 'completed' && initialized.current !== result.id) {
            initialized.current = result.id;
            setSelected(
              new Set(
                result.candidates
                  .filter(
                    (candidate) =>
                      !candidate.imported && !candidate.parent_path && !candidate.needs_review,
                  )
                  .map((candidate) => candidate.id),
              ),
            );
          }
        })
        .catch((error) => {
          if (!cancelled) {
            setLocalError(message(error));
            clearInterval(timer);
          }
        })
        .finally(() => {
          inFlight = false;
        });
    }, 350);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [progress?.id, progress?.status]);

  function changeRoot(value: string) {
    setRoot(value);
    if (progress) {
      if (idRef.current) void call('scan_cancel', { id: idRef.current }).catch(() => {});
      idRef.current = null;
      setProgress(null);
      setSelected(new Set());
      setQuery('');
    }
  }
  function search(value: string) {
    setQuery(value);
    if (value.trim())
      setExpanded(
        new Set(
          candidates
            .filter((candidate) => matchesProject(candidate, termsOf(value)))
            .map((candidate) => candidate.id),
        ),
      );
  }
  async function start() {
    setBusy(true);
    setLocalError('');
    setProgress(null);
    setSelected(new Set());
    setQuery('');
    setExpanded(new Set());
    try {
      const id = await call<string>('scan_start', { root, depth: limited ? depth : 0 });
      idRef.current = id;
      if (closingRef.current) {
        await call('scan_cancel', { id });
        return;
      }
      setProgress({
        id,
        status: 'scanning',
        visited: 0,
        entries: 0,
        current_path: root,
        candidates: [],
        warnings: [],
        incomplete: false,
      });
    } catch (error) {
      setLocalError(message(error));
    } finally {
      setBusy(false);
    }
  }
  function toggle(id: string) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  return (
    <Modal
      title={t('发现本地项目')}
      subtitle={t('自动识别项目边界，预览内部服务与共享组件后再导入。')}
      wide
      className="scan-dialog"
      close={close}
    >
      <div className="modal-body scan-config">
        <label htmlFor="scan-root">{t('扫描目录')}</label>
        <div className="directory-input">
          <Folder size={17} />
          <input
            id="scan-root"
            placeholder="/Users/you/Projects"
            value={root}
            onChange={(event) => changeRoot(event.target.value)}
            disabled={scanning || busy}
          />
          <button
            className="button secondary small"
            disabled={scanning || busy}
            onClick={async () => {
              try {
                const value = await open({
                  directory: true,
                  multiple: false,
                  title: t('选择项目扫描目录'),
                });
                if (typeof value === 'string') changeRoot(value);
              } catch (error) {
                setLocalError(message(error));
              }
            }}
          >
            {t('选择目录')}
          </button>
        </div>
        <div className="scan-mode">
          <span>
            {limited
              ? t('深度补扫 · 最多 {{0}} 层', [depth])
              : t('按项目特征与模块声明发现本机服务')}
          </span>
          <span>
            <ShieldCheck size={14} />
            {t('只读扫描，不执行命令')}
          </span>
        </div>
        <details className="scan-advanced">
          <summary>{t('高级设置')}</summary>
          <div className="scan-options">
            <label>
              <input
                type="checkbox"
                checked={limited}
                onChange={(event) => setLimited(event.target.checked)}
                disabled={scanning || busy}
              />
              {t('深度补扫非标准目录')}
            </label>
            {limited && (
              <Select
                aria-label={t('扫描深度')}
                value={String(depth)}
                onValueChange={(value) => setDepth(Number(value))}
                disabled={scanning || busy}
                options={Array.from({ length: 12 }, (_, index) => ({
                  value: String(index + 1),
                  label: t('{{0}} 层', [index + 1]),
                }))}
              />
            )}
          </div>
          <p className="field-hint">
            {t(
              '默认识别本机服务、开发服务器及相关组件，过滤纯原生客户端。非标准目录可补扫；补扫仍过滤客户端，并跳过依赖、构建和缓存，不跟随符号链接。',
            )}
          </p>
        </details>
        {localError && (
          <div className="error-banner" role="alert">
            {t(localError)}
          </div>
        )}
      </div>
      {!progress ? (
        <div className="scan-idle">
          <FolderSearch size={35} strokeWidth={1.3} />
          <p>
            {t(
              '支持 JS/TS、Java/Kotlin、Python、Rust、Go、.NET、PHP、Ruby、Dart/Flutter、Swift 和 C/C++',
            )}
          </p>
          <span>{t('仅收录本机服务与相关组件；纯移动端、桌面 GUI 和游戏工程不单独收录。')}</span>
        </div>
      ) : (
        <div className="scan-results">
          <div className="scan-result-head">
            <span>
              {scanning ? (
                <>
                  <LoaderCircle className="spin" size={15} />
                  {t('正在识别项目边界')}
                </>
              ) : completed ? (
                <>
                  <CheckCheck size={16} />
                  {progress.incomplete ? t('已发现部分结果') : t('扫描完成')} · {candidates.length}{' '}
                  {t('个项目 ·')}
                  {serviceCount} {t('个服务候选')}
                </>
              ) : (
                t('扫描已取消')
              )}
            </span>
            <span>
              {t('已检查')}
              {progress.visited} {t('个目录 · 枚举')}
              {progress.entries} {t('个条目')}
            </span>
          </div>
          {scanning && (
            <>
              <div className="scan-progress">
                <span />
              </div>
              <div className="current-path">{shortPath(progress.current_path)}</div>
            </>
          )}
          <div className="scan-search">
            <div className="search-field">
              <Search size={16} />
              <input
                aria-label={t('搜索扫描结果')}
                placeholder={t('搜索项目、路径、技术栈或内部服务…')}
                value={query}
                onChange={(event) => search(event.target.value)}
                disabled={!completed}
              />
              {query && (
                <button
                  className="icon-button"
                  aria-label={t('清空扫描搜索')}
                  onClick={() => search('')}
                >
                  <X size={14} />
                </button>
              )}
            </div>
            <span>
              {visible.length} / {candidates.length} {t('个项目')}
            </span>
            <button
              className="text-button"
              disabled={!completed || !selected.size || busy}
              onClick={() => setSelected(new Set())}
            >
              {t('清空选择')}
            </button>
          </div>
          {completed && (
            <label className="select-all">
              <input
                ref={allRef}
                type="checkbox"
                disabled={!eligible.length || busy}
                checked={eligible.length > 0 && selectedVisible === eligible.length}
                onChange={(event) => {
                  const checked = event.target.checked;
                  setSelected((previous) => {
                    const next = new Set(previous);
                    for (const candidate of eligible) {
                      if (checked) next.add(candidate.id);
                      else next.delete(candidate.id);
                    }
                    return next;
                  });
                }}
              />
              {t('选择当前结果中的未导入项目')}
              <span>{t('独立项目按需勾选；已有档案可勾选更新线索')}</span>
            </label>
          )}
          <div className="candidate-list" role="region" aria-label={t('扫描项目结果')} tabIndex={0}>
            {visible.map((candidate) => {
              const items = components(candidate);
              const shown = terms.length ? items.filter((item) => matches(item, terms)) : items;
              const services = items.filter((item) => item.runnable).length;
              return (
                <article
                  className={`candidate-group ${selected.has(candidate.id) ? 'checked' : ''}`}
                  key={candidate.id}
                >
                  <div className="candidate">
                    <input
                      id={`candidate-${candidate.id}`}
                      type="checkbox"
                      aria-label={t('选择项目 {{0}}', [candidate.name])}
                      disabled={!completed || busy}
                      checked={selected.has(candidate.id)}
                      onChange={() => toggle(candidate.id)}
                    />
                    <ProjectGlyph type={candidate.project_type} small />
                    <div>
                      <strong>
                        <label htmlFor={`candidate-${candidate.id}`}>{candidate.name}</label>
                        {candidate.imported && (
                          <span className="candidate-state">{t('已导入 · 可更新')}</span>
                        )}
                        {candidate.parent_path && (
                          <span className="candidate-state">{t('独立嵌套项目')}</span>
                        )}
                        {candidate.needs_review && (
                          <span className="candidate-state warning">{t('需要确认')}</span>
                        )}
                      </strong>
                      <span className="candidate-path" title={candidate.canonical_path}>
                        {shortPath(candidate.canonical_path)}
                      </span>
                      <span className="candidate-evidence">
                        {t(boundaryNames[candidate.boundary]) || t('项目')} · {services}{' '}
                        {t('个服务候选')}
                        {items.length > services
                          ? t(' · {{0}} 个共享组件', [items.length - services])
                          : ''}
                      </span>
                      <Tags project={candidate} />
                    </div>
                    {items.length > 0 && (
                      <button
                        className="icon-button component-toggle"
                        aria-label={t('{{0}} {{1}} 的组件', [
                          expanded.has(candidate.id) ? t('收起') : t('展开'),
                          candidate.name,
                        ])}
                        aria-expanded={expanded.has(candidate.id)}
                        onClick={() =>
                          setExpanded((previous) => {
                            const next = new Set(previous);
                            if (next.has(candidate.id)) next.delete(candidate.id);
                            else next.add(candidate.id);
                            return next;
                          })
                        }
                      >
                        <ChevronDown size={17} />
                      </button>
                    )}
                  </div>
                  {expanded.has(candidate.id) && (
                    <ul className="candidate-components">
                      {shown.map((item) => (
                        <li key={item.canonical_path}>
                          {item.runnable ? <SquareTerminal size={16} /> : <Folder size={16} />}
                          <div>
                            <strong>
                              {item.name}
                              <span className="candidate-state">
                                {item.runnable ? t('服务候选') : t('共享组件')}
                              </span>
                              {item.needs_review && (
                                <span className="candidate-state warning">{t('需核对')}</span>
                              )}
                              {item.imported &&
                                item.canonical_path !== candidate.canonical_path && (
                                  <span className="candidate-state warning">
                                    {t('已有独立档案')}
                                  </span>
                                )}
                            </strong>
                            <code title={item.canonical_path}>
                              {shortPath(item.canonical_path)}
                            </code>
                            <span>
                              {item.evidence.join(' · ')}
                              {item.frameworks.length ? ` · ${item.frameworks.join(' / ')}` : ''}
                            </span>
                          </div>
                        </li>
                      ))}
                      {!shown.length && (
                        <li className="field-hint">
                          {t('当前关键词匹配项目信息，未匹配内部组件。')}
                        </li>
                      )}
                    </ul>
                  )}
                </article>
              );
            })}
            {completed && !visible.length && (
              <p className="no-results">
                {query
                  ? t('没有匹配的项目或组件，试试其他关键词。')
                  : t('未识别到项目，可选择更具体的目录或检查高级设置。')}
              </p>
            )}
            {scanning && (
              <p className="no-results">{t('正在收集清单与仓库信息，完成后可搜索并预览分组。')}</p>
            )}
          </div>
          {progress.warnings.length > 0 && (
            <div className="scan-warnings" role="status">
              {progress.warnings.map((warning) => (
                <p className="scan-warning" key={t(warning)}>
                  {t(warning)}
                </p>
              ))}
            </div>
          )}
        </div>
      )}
      <div className="modal-actions">
        <span className="modal-hint">
          <Database size={13} />
          {completed
            ? t('已选 {{0}} 个项目{{1}}{{2}}', [
                selected.size,
                updating ? t('，含 {{0}} 个档案更新', [updating]) : '',
                hiddenSelected ? t('（{{0}} 个未显示）', [hiddenSelected]) : '',
              ])
            : t('服务候选仅作配置线索，不自动创建或启动')}
        </span>
        <button className="button secondary" disabled={busy} onClick={close}>
          {t('取消')}
        </button>
        {scanning ? (
          <button
            className="button secondary"
            onClick={async () => {
              try {
                await call('scan_cancel', { id: progress.id });
              } catch (error) {
                report(error);
              }
            }}
          >
            {t('停止扫描')}
          </button>
        ) : (
          <>
            <button
              className={`button ${completed ? 'secondary' : 'primary'}`}
              disabled={!root.trim() || busy}
              onClick={() => void start()}
            >
              {busy && <LoaderCircle size={15} className="spin" />}
              {progress ? t('重新扫描') : t('开始扫描')}
              <ArrowRight size={15} />
            </button>
            {completed && (
              <button
                className="button primary"
                disabled={!selected.size || busy}
                onClick={async () => {
                  setBusy(true);
                  try {
                    imported(
                      await call<Project[]>('import_projects', {
                        scanId: progress.id,
                        ids: [...selected],
                      }),
                    );
                  } catch (error) {
                    setLocalError(message(error));
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                {updating ? t('导入 / 更新') : t('导入')} {selected.size} {t('个项目')}
              </button>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}

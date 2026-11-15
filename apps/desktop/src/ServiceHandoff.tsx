import { useState } from 'react';
import { ArrowRightLeft, LoaderCircle } from 'lucide-react';
import { Modal } from './components';
import Select from './Select';
import { t } from './i18n';
import { serviceIsActive } from './ServicePresentation';
import type {
  ExternalServiceConfiguration,
  PortBinding,
  ServiceDefinition,
  ServiceView,
} from './types';

export function HandoffChooser({
  service,
  choices,
  disabled,
  preview,
  close,
}: {
  service: ServiceDefinition;
  choices: PortBinding[][];
  disabled: boolean;
  preview: (service: ServiceDefinition, pid: number) => Promise<void>;
  close: () => void;
}) {
  const [pid, setPid] = useState('');
  const selected = choices.find((bindings) => String(bindings[0].pid) === pid);
  return (
    <section className="service-handoff-chooser form-stack" aria-label={t('选择外部实例切换托管')}>
      <label>
        {t('要替换的外部实例')}
        <Select
          aria-label={t('要替换的外部实例')}
          value={selected ? pid : ''}
          onValueChange={setPid}
          disabled={disabled}
          options={[
            { value: '', label: t('请选择外部实例…') },
            ...choices.map((bindings) => ({
              value: String(bindings[0].pid),
              label: `PID ${bindings[0].pid} · ${[...new Set(bindings.map((b) => ':' + b.port))].join(', ')} · ${bindings[0].cwd || t('工作目录无法读取')}`,
            })),
          ]}
        />
      </label>
      {selected && (
        <div className="service-command">
          <code>{selected[0].command}</code>
          <span>{selected[0].cwd}</span>
        </div>
      )}
      <p className="field-hint">
        {t(
          '同项目的监听仅供选择，不代表已关联或可以停止。下一步由 Core 重新核验身份，展示旧实例和现有配置，再单独确认。',
        )}
      </p>
      <div className="service-guide-actions">
        <button
          type="button"
          className="button primary small"
          disabled={disabled || !selected}
          onClick={() => selected && void preview(service, selected[0].pid)}
        >
          {disabled ? <LoaderCircle size={14} className="spin" /> : <ArrowRightLeft size={14} />}
          {t('预览切换')}
        </button>
        <button type="button" className="text-button" disabled={disabled} onClick={close}>
          {t('暂不切换')}
        </button>
      </div>
    </section>
  );
}

export function ExistingConfigurationChoice({
  setup,
  services,
  busy,
  disabled,
  preview,
  create,
  select,
  close,
}: {
  setup: ExternalServiceConfiguration;
  services: ServiceView[];
  busy: boolean;
  disabled: boolean;
  preview: (service: ServiceDefinition, pid: number) => Promise<void>;
  create: () => void;
  select: (id: string) => void;
  close: () => void;
}) {
  const [id, setId] = useState(
    services.length === 1 && !serviceIsActive(services[0].current_run?.state)
      ? services[0].definition.id
      : '',
  );
  const selected = services.find((v) => v.definition.id === id);
  const usable = selected && !serviceIsActive(selected.current_run?.state);
  return (
    <Modal
      title={t('使用已有服务配置')}
      subtitle={t('此项目已有配置，可直接预览切换，无需重复保存。选择不会停止外部实例。')}
      className="service-confirm-dialog"
      close={() => !busy && close()}
    >
      <div className="modal-body form-stack">
        <strong>
          {t('外部实例：')}PID {setup.process.pid} ·{' '}
          {[...new Set(setup.process.bindings.map((b) => ':' + b.port))].join(', ')}
        </strong>
        <div className="service-command">
          <code>{setup.process.command}</code>
          <span>{setup.process.cwd}</span>
        </div>
        <label>
          {t('选择已有配置')}
          <Select
            aria-label={t('选择已有配置')}
            value={selected ? id : ''}
            onValueChange={setId}
            disabled={busy}
            options={[
              { value: '', label: t('请选择服务配置…') },
              ...services.map((v) => ({
                value: v.definition.id,
                label: `${v.definition.name} · ${v.definition.working_directory}${serviceIsActive(v.current_run?.state) ? ' · ' + t('已有活动运行') : ''}`,
                disabled: serviceIsActive(v.current_run?.state),
              })),
            ]}
          />
        </label>
        {selected && (
          <div className="service-command">
            <code>{[selected.definition.executable, ...selected.definition.args].join(' ')}</code>
            <span>{selected.definition.working_directory}</span>
          </div>
        )}
        {setup.handoff_supported === false && (
          <p className="field-hint">
            {t('当前后台不支持切换托管；可先查看已有配置，更新 Core 后再预览。')}
          </p>
        )}
        <button type="button" className="text-button" disabled={busy} onClick={create}>
          {t('新建另一份配置…')}
        </button>
      </div>
      <div className="modal-actions">
        <button className="button secondary" disabled={busy} onClick={close}>
          {t('取消')}
        </button>
        <button
          className="button secondary"
          disabled={busy || !selected}
          onClick={() => selected && select(selected.definition.id)}
        >
          {t('查看已有配置')}
        </button>
        <button
          className="button primary"
          disabled={busy || disabled || !usable || setup.handoff_supported === false}
          onClick={() => usable && void preview(selected.definition, setup.process.pid)}
        >
          {t('预览切换')}
        </button>
      </div>
    </Modal>
  );
}

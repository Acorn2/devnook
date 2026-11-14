import { t } from './i18n';
import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';

type SelectOption = { value: string; label: string; disabled?: boolean };
type SelectProps = {
  id?: string;
  'aria-label': string;
  'aria-describedby'?: string;
  value: string;
  options: SelectOption[];
  onValueChange: (value: string) => void;
  disabled?: boolean;
  required?: boolean;
  placeholder?: string;
  displayLabel?: string;
};

export default function Select({
  id,
  'aria-label': label,
  'aria-describedby': describedBy,
  value,
  options,
  onValueChange,
  disabled = false,
  required = false,
  placeholder = t('请选择'),
  displayLabel,
}: SelectProps) {
  const generatedId = useId();
  const triggerId = id || generatedId;
  const listId = `${generatedId}-options`;
  const errorId = `${generatedId}-error`;
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const typeahead = useRef({ text: '', at: 0 });
  const [open, setOpen] = useState(false);
  const [activeValue, setActiveValue] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  const [position, setPosition] = useState<{
    left: number;
    top: number;
    width: number;
    maxHeight: number;
  } | null>(null);
  const selected = options.find((option) => option.value === value);
  const enabled = options.filter((option) => !option.disabled);
  const unavailable = disabled || enabled.length === 0;
  const active = enabled.find((option) => option.value === activeValue) || enabled[0];
  const activeIndex = options.findIndex((option) => option === active);

  function show(firstValue = selected && !selected.disabled ? selected.value : enabled[0]?.value) {
    if (unavailable) return;
    typeahead.current = { text: '', at: 0 };
    setActiveValue(firstValue ?? null);
    setPosition(null);
    setOpen(true);
  }

  function choose(nextValue: string) {
    if (unavailable || !enabled.some((option) => option.value === nextValue)) return;
    setOpen(false);
    setInvalid(required && !nextValue);
    if (nextValue !== value) onValueChange(nextValue);
  }

  useEffect(() => {
    if (unavailable) setOpen(false);
    if (value) setInvalid(false);
  }, [unavailable, value]);

  useLayoutEffect(() => {
    if (!open || unavailable || !trigger.current || !menu.current) return;
    const bounds = trigger.current.getBoundingClientRect();
    const fontSize = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
    const inset = 8;
    const gap = 6;
    const width = Math.min(Math.max(bounds.width, 16 * fontSize), innerWidth - inset * 2);
    menu.current.style.width = `${width}px`;
    const borders = menu.current.offsetHeight - menu.current.clientHeight;
    const desiredHeight = Math.min(menu.current.scrollHeight + borders, 20 * fontSize);
    const below = innerHeight - bounds.bottom - gap - inset;
    const above = bounds.top - gap - inset;
    const upwards = below < desiredHeight && above > below;
    const maxHeight = Math.max(0, Math.min(desiredHeight, upwards ? above : below));
    setPosition({
      width,
      maxHeight,
      left: Math.max(inset, Math.min(bounds.left, innerWidth - width - inset)),
      top: upwards ? bounds.top - gap - maxHeight : bounds.bottom + gap,
    });
  }, [open, unavailable, options]);

  useLayoutEffect(() => {
    if (!open || !position || !menu.current) return;
    const option = menu.current.querySelectorAll<HTMLElement>('[role="option"]')[activeIndex];
    if (!option) return;
    const top = option.offsetTop;
    const bottom = top + option.offsetHeight;
    if (top < menu.current.scrollTop) menu.current.scrollTop = top;
    else if (bottom > menu.current.scrollTop + menu.current.clientHeight) {
      menu.current.scrollTop = bottom - menu.current.clientHeight;
    }
  }, [open, position, activeIndex]);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !trigger.current?.contains(event.target) &&
        !menu.current?.contains(event.target)
      ) {
        setOpen(false);
      }
    };
    const scroll = (event: Event) => {
      if (event.target instanceof Node && menu.current?.contains(event.target)) return;
      setOpen(false);
    };
    const resize = () => setOpen(false);
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', resize);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('scroll', scroll, true);
      window.removeEventListener('resize', resize);
    };
  }, [open]);

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (unavailable || event.nativeEvent.isComposing) return;
    const index = enabled.findIndex((option) => option === active);
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
    } else if (event.key === 'Tab') {
      setOpen(false);
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      if (open && active) choose(active.value);
      else show();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      if (!open) show();
      else if (event.altKey && event.key === 'ArrowUp' && active) choose(active.value);
      else {
        const nextIndex = Math.max(
          0,
          Math.min(enabled.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1)),
        );
        setActiveValue(enabled[nextIndex].value);
      }
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      const option = enabled[event.key === 'Home' ? 0 : enabled.length - 1];
      if (!open) show(option.value);
      else setActiveValue(option.value);
    } else if (open && (event.key === 'PageDown' || event.key === 'PageUp')) {
      event.preventDefault();
      setActiveValue(
        enabled[
          Math.max(0, Math.min(enabled.length - 1, index + (event.key === 'PageDown' ? 10 : -10)))
        ].value,
      );
    } else if (event.key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      const now = Date.now();
      const previous = now - typeahead.current.at < 700 ? typeahead.current.text : '';
      const text = previous + event.key.toLocaleLowerCase();
      typeahead.current = { text, at: now };
      const repeated = [...text].every((character) => character === text[0]);
      const prefix = repeated ? text[0] : text;
      const start = open ? index : enabled.findIndex((option) => option.value === value);
      const order = [...enabled.slice(start + 1), ...enabled.slice(0, start + 1)];
      const match = order.find((option) => option.label.toLocaleLowerCase().startsWith(prefix));
      if (!open) {
        setPosition(null);
        setOpen(true);
      }
      if (match) setActiveValue(match.value);
    }
  }

  return (
    <div className="select-control">
      <button
        ref={trigger}
        id={triggerId}
        type="button"
        role="combobox"
        className="select-trigger"
        disabled={unavailable}
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open && activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined}
        aria-required={required || undefined}
        aria-invalid={invalid || undefined}
        aria-describedby={
          [describedBy, invalid ? errorId : ''].filter(Boolean).join(' ') || undefined
        }
        data-open={open}
        data-placeholder={!selected}
        title={selected?.label || placeholder}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
        onBlur={() => setOpen(false)}
      >
        <span className="select-value">{displayLabel ?? (selected?.label || placeholder)}</span>
        <ChevronDown size={15} className="select-chevron" aria-hidden="true" />
      </button>
      {required && (
        <select
          hidden
          tabIndex={-1}
          aria-hidden="true"
          required
          disabled={disabled}
          value={value}
          onChange={(event) => choose(event.target.value)}
          onInvalid={(event) => {
            event.preventDefault();
            setInvalid(true);
            trigger.current?.focus();
          }}
        >
          {!options.some((option) => option.value === '') && (
            <option value="">{placeholder}</option>
          )}
          {options.map((option) => (
            <option key={option.value} value={option.value} disabled={option.disabled}>
              {option.label}
            </option>
          ))}
        </select>
      )}
      {invalid && (
        <span id={errorId} className="select-error" role="alert">
          {t('请选择一项')}
        </span>
      )}
      {open &&
        !unavailable &&
        createPortal(
          <div
            ref={menu}
            id={listId}
            role="listbox"
            aria-label={label}
            className="select-menu"
            style={position || { visibility: 'hidden' }}
            onPointerDown={(event) => event.preventDefault()}
          >
            {options.map((option, index) => (
              <div
                id={`${listId}-${index}`}
                key={option.value}
                role="option"
                aria-selected={option.value === value}
                aria-disabled={option.disabled || undefined}
                className="select-option"
                data-active={option === active}
                onPointerMove={() => !option.disabled && setActiveValue(option.value)}
                onClick={() => choose(option.value)}
              >
                <span>{option.label}</span>
                {option.value === value && <Check size={15} aria-hidden="true" />}
              </div>
            ))}
          </div>,
          trigger.current?.closest('dialog') || document.body,
        )}
    </div>
  );
}

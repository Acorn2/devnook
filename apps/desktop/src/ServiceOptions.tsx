import { useEffect, useRef, type ReactNode } from 'react';
import { MoreHorizontal } from 'lucide-react';

export default function ServiceOptions({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  const root = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target) && root.current)
        root.current.open = false;
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, []);
  return (
    <details
      ref={root}
      className="service-options"
      onBlur={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          !event.currentTarget.contains(event.relatedTarget)
        )
          event.currentTarget.open = false;
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && event.currentTarget.open) {
          event.preventDefault();
          event.stopPropagation();
          event.currentTarget.open = false;
          event.currentTarget.querySelector('summary')?.focus();
        }
      }}
    >
      <summary className="icon-button" aria-label={label} title={label}>
        <MoreHorizontal size={17} />
      </summary>
      <div
        className="service-options-content"
        role="group"
        aria-label={label}
        onClick={(event) => {
          const button = (event.target as HTMLElement).closest('button');
          if (button && !button.disabled && root.current) {
            root.current.open = false;
            root.current.querySelector('summary')?.focus();
          }
        }}
      >
        {children}
      </div>
    </details>
  );
}

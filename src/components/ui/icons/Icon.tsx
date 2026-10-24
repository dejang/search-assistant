import * as React from 'react';
import { ICONS, type IconName } from './paths';

export interface IconProps extends React.SVGAttributes<SVGSVGElement> {
  name: IconName;
  /** Rendered width and height in px. Default 16. */
  size?: number;
  /** Accessible title. Omit for decorative icons (aria-hidden). */
  title?: string;
}

/** Pane 16px line icons (spec Appendix B).
 * Takes context color: accent in sidebars, label-secondary in toolbars, label in content. */
export function Icon({ name, size = 16, title, ...rest }: IconProps) {
  const elements = ICONS[name];
  return (
    <svg
      viewBox="0 0 16 16"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      focusable="false"
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      {...rest}
    >
      {title ? <title>{title}</title> : null}
      {elements.map(([tag, attrs], i) => {
        const { fill, knockout, ...attributes } = attrs;
        const style = fill
          ? { fill: 'currentColor', stroke: 'none' }
          : knockout
            ? { stroke: 'var(--field-bg)' }
            : undefined;
        return React.createElement(tag, { key: i, ...attributes, style });
      })}
    </svg>
  );
}

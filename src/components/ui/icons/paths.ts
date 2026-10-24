/* Pane icon set (spec Appendix B): 32 line icons on a 16×16 grid.
   fill: 1 uses currentColor fill; knockout: 1 strokes with --field-bg. */

export type IconElementAttrs = {
  [attribute: string]: string | number | 1 | undefined;
  /** Fill with currentColor and no stroke. */
  fill?: 1;
  /** Stroke with var(--field-bg). */
  knockout?: 1;
};

export type IconElement =
  | ['path', IconElementAttrs]
  | ['circle', IconElementAttrs]
  | ['rect', IconElementAttrs];

export type IconName =
  | 'sidebar'
  | 'chevron-left'
  | 'chevron-right'
  | 'chevron-down'
  | 'chevron-up-down'
  | 'search'
  | 'plus'
  | 'minus'
  | 'xmark'
  | 'xmark-circle'
  | 'checkmark'
  | 'folder'
  | 'doc'
  | 'gear'
  | 'trash'
  | 'share'
  | 'star'
  | 'clock'
  | 'person'
  | 'bell'
  | 'tag'
  | 'info'
  | 'warning'
  | 'ellipsis'
  | 'list'
  | 'grid'
  | 'download'
  | 'cloud'
  | 'lock'
  | 'globe'
  | 'photo'
  | 'house';

export const ICONS: Record<IconName, IconElement[]> = {
  sidebar: [
    ['rect', { x: 1.75, y: 2.75, width: 12.5, height: 10.5, rx: 2 }],
    ['path', { d: 'M6 2.75 V13.25' }],
    ['path', { d: 'M3.5 5.25 H4.25 M3.5 7.25 H4.25' }],
  ],
  'chevron-left': [['path', { d: 'M10 3.5 L5.5 8 L10 12.5' }]],
  'chevron-right': [['path', { d: 'M6 3.5 L10.5 8 L6 12.5' }]],
  'chevron-down': [['path', { d: 'M3.5 6 L8 10.5 L12.5 6' }]],
  'chevron-up-down': [['path', { d: 'M5 6.25 L8 3.25 L11 6.25 M5 9.75 L8 12.75 L11 9.75' }]],
  search: [
    ['circle', { cx: 7, cy: 7, r: 4.5 }],
    ['path', { d: 'M10.4 10.4 L13.75 13.75' }],
  ],
  plus: [['path', { d: 'M8 3 V13 M3 8 H13' }]],
  minus: [['path', { d: 'M3 8 H13' }]],
  xmark: [['path', { d: 'M4 4 L12 12 M12 4 L4 12' }]],
  'xmark-circle': [
    ['circle', { cx: 8, cy: 8, r: 6.5, fill: 1 }],
    ['path', { d: 'M5.75 5.75 L10.25 10.25 M10.25 5.75 L5.75 10.25', knockout: 1 }],
  ],
  checkmark: [['path', { d: 'M3.25 8.5 L6.5 11.75 L12.75 4.25' }]],
  folder: [
    [
      'path',
      {
        d: 'M1.75 4.5 A1.75 1.75 0 0 1 3.5 2.75 H6.25 L7.75 4.25 H12.5 A1.75 1.75 0 0 1 14.25 6 V11.5 A1.75 1.75 0 0 1 12.5 13.25 H3.5 A1.75 1.75 0 0 1 1.75 11.5 Z',
      },
    ],
    ['path', { d: 'M1.75 6.5 H14.25' }],
  ],
  doc: [
    [
      'path',
      {
        d: 'M4.25 1.75 H9.25 L12.75 5.25 V13 A1.25 1.25 0 0 1 11.5 14.25 H4.25 A1.25 1.25 0 0 1 3 13 V3 A1.25 1.25 0 0 1 4.25 1.75 Z',
      },
    ],
    ['path', { d: 'M9.25 1.75 V5.25 H12.75' }],
    ['path', { d: 'M5.5 8.5 H10.25 M5.5 11 H8.75' }],
  ],
  gear: [
    ['circle', { cx: 8, cy: 8, r: 4.6 }],
    ['circle', { cx: 8, cy: 8, r: 1.9 }],
    [
      'path',
      {
        d: 'M12.60 8.00 L14.40 8.00 M11.25 11.25 L12.53 12.53 M8.00 12.60 L8.00 14.40 M4.75 11.25 L3.47 12.53 M3.40 8.00 L1.60 8.00 M4.75 4.75 L3.47 3.47 M8.00 3.40 L8.00 1.60 M11.25 4.75 L12.53 3.47',
      },
    ],
  ],
  trash: [
    ['path', { d: 'M2.75 4.25 H13.25' }],
    ['path', { d: 'M6.25 4.25 V3 A0.75 0.75 0 0 1 7 2.25 H9 A0.75 0.75 0 0 1 9.75 3 V4.25' }],
    ['path', { d: 'M4 4.25 L4.65 12.9 A1.25 1.25 0 0 0 5.9 14.05 H10.1 A1.25 1.25 0 0 0 11.35 12.9 L12 4.25' }],
    ['path', { d: 'M6.75 7 V11.25 M9.25 7 V11.25' }],
  ],
  share: [
    ['path', { d: 'M8 1.75 V9.75' }],
    ['path', { d: 'M5.25 4.5 L8 1.75 L10.75 4.5' }],
    [
      'path',
      {
        d: 'M5.5 6.75 H4.25 A1 1 0 0 0 3.25 7.75 V13.25 A1 1 0 0 0 4.25 14.25 H11.75 A1 1 0 0 0 12.75 13.25 V7.75 A1 1 0 0 0 11.75 6.75 H10.5',
      },
    ],
  ],
  star: [
    [
      'path',
      {
        d: 'M8.00 2.20 L9.62 6.38 L14.09 6.62 L10.62 9.45 L11.76 13.78 L8.00 11.35 L4.24 13.78 L5.38 9.45 L1.91 6.62 L6.38 6.38 Z',
      },
    ],
  ],
  clock: [
    ['circle', { cx: 8, cy: 8, r: 6.25 }],
    ['path', { d: 'M8 4.5 V8 L10.5 9.75' }],
  ],
  person: [
    ['circle', { cx: 8, cy: 5.5, r: 2.75 }],
    ['path', { d: 'M2.75 14 A5.25 5.25 0 0 1 13.25 14' }],
  ],
  bell: [
    ['path', { d: 'M4 11.25 V7.25 A4 4 0 0 1 12 7.25 V11.25 L13.25 12.5 H2.75 Z' }],
    ['path', { d: 'M6.5 14.25 H9.5' }],
  ],
  tag: [
    [
      'path',
      {
        d: 'M1.75 2.5 V7.4 A1 1 0 0 0 2.05 8.1 L8.15 14.2 A1 1 0 0 0 9.55 14.2 L14.2 9.55 A1 1 0 0 0 14.2 8.15 L8.1 2.05 A1 1 0 0 0 7.4 1.75 H2.5 A0.75 0.75 0 0 0 1.75 2.5 Z',
      },
    ],
    ['circle', { cx: 5, cy: 5, r: 1.1, fill: 1 }],
  ],
  info: [
    ['circle', { cx: 8, cy: 8, r: 6.25 }],
    ['path', { d: 'M8 7.25 V11.25' }],
    ['circle', { cx: 8, cy: 4.85, r: 0.85, fill: 1 }],
  ],
  warning: [
    ['path', { d: 'M7.13 2.75 A1 1 0 0 1 8.87 2.75 L14.1 12.25 A1 1 0 0 1 13.23 13.75 H2.77 A1 1 0 0 1 1.9 12.25 Z' }],
    ['path', { d: 'M8 6.25 V9.25' }],
    ['circle', { cx: 8, cy: 11.4, r: 0.85, fill: 1 }],
  ],
  ellipsis: [
    ['circle', { cx: 3.5, cy: 8, r: 1.15, fill: 1 }],
    ['circle', { cx: 8, cy: 8, r: 1.15, fill: 1 }],
    ['circle', { cx: 12.5, cy: 8, r: 1.15, fill: 1 }],
  ],
  list: [
    ['path', { d: 'M5.75 4 H13.5 M5.75 8 H13.5 M5.75 12 H13.5' }],
    ['circle', { cx: 2.75, cy: 4, r: 0.9, fill: 1 }],
    ['circle', { cx: 2.75, cy: 8, r: 0.9, fill: 1 }],
    ['circle', { cx: 2.75, cy: 12, r: 0.9, fill: 1 }],
  ],
  grid: [
    ['rect', { x: 2, y: 2, width: 5, height: 5, rx: 1.25 }],
    ['rect', { x: 9, y: 2, width: 5, height: 5, rx: 1.25 }],
    ['rect', { x: 2, y: 9, width: 5, height: 5, rx: 1.25 }],
    ['rect', { x: 9, y: 9, width: 5, height: 5, rx: 1.25 }],
  ],
  download: [
    ['path', { d: 'M8 2 V10.5' }],
    ['path', { d: 'M4.75 7.25 L8 10.5 L11.25 7.25' }],
    ['path', { d: 'M2.75 13.75 H13.25' }],
  ],
  cloud: [['path', { d: 'M4.6 12.75 H11.6 A3 3 0 0 0 12.1 6.8 A4.1 4.1 0 0 0 4.35 7.55 A2.6 2.6 0 0 0 4.6 12.75 Z' }]],
  lock: [
    ['rect', { x: 3.25, y: 7, width: 9.5, height: 7, rx: 1.75 }],
    ['path', { d: 'M5.25 7 V5 A2.75 2.75 0 0 1 10.75 5 V7' }],
  ],
  globe: [
    ['circle', { cx: 8, cy: 8, r: 6.25 }],
    ['path', { d: 'M8 1.75 C5.25 4.5 5.25 11.5 8 14.25 C10.75 11.5 10.75 4.5 8 1.75 Z' }],
    ['path', { d: 'M1.75 8 H14.25' }],
  ],
  photo: [
    ['rect', { x: 1.75, y: 2.75, width: 12.5, height: 10.5, rx: 1.75 }],
    ['circle', { cx: 5.5, cy: 6.25, r: 1.15 }],
    ['path', { d: 'M1.75 11.25 L5.25 8 L8.25 10.75 L10.25 9 L14.25 12.5' }],
  ],
  house: [
    ['path', { d: 'M2.25 7.25 L8 2.25 L13.75 7.25' }],
    ['path', { d: 'M3.75 6 V13.25 H12.25 V6' }],
    ['path', { d: 'M6.75 13.25 V9.75 H9.25 V13.25' }],
  ],
};

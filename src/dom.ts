export type Child = Node | string | null | false | undefined;
export type Prop = string | boolean | null | undefined | ((event: Event) => void);
export type Props = Readonly<Record<string, Prop>>;

const PROPERTY_PROPS: ReadonlySet<string> = new Set(['value', 'disabled', 'type', 'hidden']);

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Props = {},
  ...children: readonly Child[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(props)) {
    if (value === null || value === undefined || value === false) continue;
    if (typeof value === 'function') {
      element.addEventListener(name.slice(2), value);
    } else if (name === 'class') {
      element.className = String(value);
    } else if (name === 'key') {
      element.dataset.key = String(value);
    } else if (name === 'text') {
      element.textContent = String(value);
    } else if (PROPERTY_PROPS.has(name)) {
      Reflect.set(element, name, value);
    } else {
      element.setAttribute(name, value === true ? '' : value);
    }
  }
  for (const child of children) {
    if (child !== null && child !== undefined && child !== false) element.append(child);
  }
  return element;
}

export function icon(name: 'ok' | 'error' | 'more'): SVGSVGElement {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'icon');
  svg.setAttribute('aria-hidden', 'true');
  const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
  use.setAttribute('href', `#i-${name}`);
  svg.append(use);
  return svg;
}

export function focusedKey(root: HTMLElement): string | null {
  const active = document.activeElement;
  if (active instanceof HTMLElement && root.contains(active)) return active.dataset.key ?? null;
  return null;
}

export interface RestoreFocusOptions {
  readonly select?: boolean;
}

export function restoreFocus(
  root: HTMLElement,
  key: string | null,
  options: RestoreFocusOptions = {},
): void {
  if (key === null) return;
  const target = root.querySelector(`[data-key="${CSS.escape(key)}"]`);
  if (!(target instanceof HTMLElement) || ('disabled' in target && target.disabled === true)) {
    return;
  }
  target.focus();
  if (target instanceof HTMLInputElement) {
    if (options.select === true) target.select();
    else target.setSelectionRange(target.value.length, target.value.length);
  }
}

export function revealNearest(element: Element | null): void {
  element?.scrollIntoView({ block: 'nearest' });
}

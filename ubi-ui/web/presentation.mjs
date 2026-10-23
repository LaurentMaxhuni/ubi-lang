import { MotionState, normalizeTargets } from './motion.mjs';

const motionModes = new Set(['system', 'full', 'reduced']);
const editableKinds = new Set(['TextInput', 'PasswordInput', 'TextArea', 'NumberInput']);
const tabPrefix = '@ubi-tab:';
const feedback = { durationMs: 140, delayMs: 0, easing: 'EaseOut' };
const selection = { durationMs: 220, delayMs: 0, easing: 'Spring' };
const disclosure = { durationMs: 220, delayMs: 0, easing: 'EaseInOut' };
const overlayEntrance = { durationMs: 220, delayMs: 0, easing: 'EaseOut' };
const clamp = (value, min, max) => Math.min(max, Math.max(min, value));
const formatTicker = value => {
  const bounded=clamp(value,-2147483648,2147483647);
  return String(Math.sign(bounded)*Math.floor(Math.abs(bounded)+0.5));
};
const enumValue = value => typeof value === 'string' ? value : value?.tag;
const editableSelector = '.ubi-textinput,.ubi-passwordinput,.ubi-textarea,.ubi-numberinput';

function getClock(root, custom) {
  if (custom !== undefined) {
    if (!custom || typeof custom.now !== 'function' || typeof custom.requestFrame !== 'function' || typeof custom.cancelFrame !== 'function') {
      throw new TypeError('Invalid Ubi host clock');
    }
    return custom;
  }
  const view = root.ownerDocument.defaultView;
  return {
    now: () => view.performance.now(),
    requestFrame: callback => view.requestAnimationFrame(callback),
    cancelFrame: id => view.cancelAnimationFrame(id),
  };
}

function defaultMotion(node) {
  if (node.kind === 'Button' || node.kind === 'IconButton' || node.kind === 'Link') return feedback;
  if (node.kind === 'Switch' || node.kind === 'Tab' || node.kind === 'ToggleItem' || node.kind === 'SegmentedControl') return selection;
  if (node.kind === 'ProgressBar') return feedback;
  if (node.props.Disclosure !== undefined) return disclosure;
  if (node.props.Effect === 'NumberTicker' || node.props.Effect === 'TextReveal' || node.props.Effect === 'AnimatedList') return overlayEntrance;
  if (node.props.Enter !== undefined || node.props.Exit !== undefined || node.kind === 'Overlay') return overlayEntrance;
  return { durationMs: 0, delayMs: 0, easing: 'EaseOut' };
}

function nodeChannels(node) {
  const props = node.props;
  const translate = props.Translate ?? [0, 0];
  const channels = {
    opacity: props.Opacity === undefined ? 1 : props.Opacity / 100,
    'translate-x': translate[0], 'translate-y': translate[1],
    scale: props.Scale === undefined ? 1 : props.Scale / 100,
  };
  if (node.kind === 'Switch') channels['switch-position'] = props.Checked ? 1 : 0;
  if (node.kind === 'ProgressBar') channels.progress = props.Progress;
  if (props.Effect === 'NumberTicker') channels.number = props.Number;
  if (props.Disclosure !== undefined) channels['disclosure-height'] = 0;
  if (node.kind === 'Overlay') channels['overlay-open'] = props.Open ? 1 : 0;
  return channels;
}

function snapshotTree(node, byKey, seen = new Set()) {
  if (seen.has(node.key) || editableKinds.has(node.kind)) return null;
  seen.add(node.key);
  const children = node.children.map(key => byKey.get(key)).filter(Boolean).map(child => snapshotTree(child, byKey, seen)).filter(Boolean);
  seen.delete(node.key);
  return { ...node, props: { ...node.props }, style: { ...node.style }, children };
}

function makeTargets(view) {
  const byKey = new Map(view.nodes.map(node => [node.key, node]));
  const parentByKey = new Map();
  for (const parent of view.nodes) for (const key of parent.children) parentByKey.set(key, parent);
  const contextFor = node => {
    let parent = parentByKey.get(node.key), childKey = node.key, underClosed = false, underDisclosure = false, underOverlay = false, listIndex = null;
    while (parent) {
      if (parent.kind === 'Overlay') { underOverlay = true; if (!parent.props.Open) underClosed = true; }
      if (parent.props.Disclosure === false) { underDisclosure = true; underClosed = true; }
      if (parent.props.Disclosure === true) underDisclosure = true;
      if (parent.props.Effect === 'AnimatedList' && childKey === node.key) listIndex = Math.max(0, parent.children.indexOf(childKey));
      childKey = parent.key;
      parent = parentByKey.get(parent.key);
    }
    return { underClosed, underDisclosure, underOverlay, listIndex };
  };
  return view.nodes.filter(node => {
    const context = contextFor(node);
    return !(node.kind === 'Overlay' && !node.props.Open) && !context.underClosed;
  }).map(node => {
    const context = contextFor(node);
    const enter = node.props.Effect === 'TextReveal' ? 'None' : enumValue(node.props.Enter) ?? (context.listIndex !== null ? 'Rise' : context.underDisclosure ? 'Fade' : node.kind === 'Overlay' ? 'Fade' : 'None');
    const exit = enumValue(node.props.Exit) ?? (context.listIndex !== null ? 'Fade' : context.underDisclosure && !context.underOverlay ? 'Fade' : context.underOverlay ? 'None' : node.kind === 'Overlay' ? 'Fade' : 'None');
    const listMotion = context.listIndex === null ? null : { ...overlayEntrance, delayMs: Math.min(context.listIndex, 12) * 32 };
    return {
      key: node.key, kind: node.kind, channels: nodeChannels(node),
      motion: node.props.Motion ?? (context.underDisclosure ? disclosure : listMotion ?? defaultMotion(node)),
      enter, exit,
      snapshot: exit === 'None' ? null : snapshotTree(node, byKey),
    };
  });
}

function stripSnapshotClone(clone) {
  if (clone.matches?.(editableSelector)) return null;
  clone.querySelectorAll?.(editableSelector).forEach(element => element.remove());
  clone.querySelectorAll?.('.ubi-tab-indicator,.ubi-exit-layer,.ubi-disclosure-exit').forEach(element=>element.remove());
  const elements = [clone, ...(clone.querySelectorAll?.('*') ?? [])];
  for (const element of elements) {
    for (const name of element.getAttributeNames?.() ?? []) {
      if (name === 'id' || name === 'data-ubi-key' || name === 'name' || name === 'for' || name === 'tabindex' ||
          name === 'role' || name.startsWith('aria-') || name.startsWith('on')) element.removeAttribute(name);
    }
    element.removeAttribute?.('disabled');
    if ('inert' in element) element.inert = true;
  }
  clone.setAttribute('aria-hidden', 'true');
  return clone;
}

function createExitLayer(root, entry, channels) {
  if (!entry?.element || editableKinds.has(entry.kind)) return null;
  const rect = entry.element.getBoundingClientRect();
  if (!rect.width || !rect.height) return null;
  const clone = stripSnapshotClone(entry.element.cloneNode(true));
  if (!clone) return null;
  Object.assign(clone.style,{position:'relative',left:'auto',top:'auto',right:'auto',bottom:'auto',width:'100%',height:'100%',maxWidth:'none',maxHeight:'none',transform:'none',zIndex:'auto',pointerEvents:'none'});
  const document = root.ownerDocument;
  const layer = document.createElement('div');
  layer.className = 'ubi-exit-layer';
  layer.setAttribute('aria-hidden', 'true');
  layer.inert = true;
  Object.assign(layer.style, {
    left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`,
  });
  layer.dataset.ubiExit = entry.element.getAttribute('data-ubi-key') ?? '';
  const base = { opacity: 1, 'translate-x': 0, 'translate-y': 0, scale: 1, ...(channels ?? {}) };
  clone.style.setProperty('--ubi-motion-opacity', String(base.opacity));
  clone.style.setProperty('--ubi-motion-x', '0px');
  clone.style.setProperty('--ubi-motion-y', '0px');
  clone.style.setProperty('--ubi-motion-scale', '1');
  clone.style.removeProperty('--ubi-effect-rotate-x');
  clone.style.removeProperty('--ubi-effect-rotate-y');
  layer.append(clone);
  return { layer, clone, base };
}

function setMotionStyle(entry, channels) {
  const element = entry.element;
  element.style.setProperty('--ubi-motion-opacity', String(clamp(channels.opacity ?? 1, 0, 1)));
  element.style.setProperty('--ubi-motion-x', `${clamp(channels['translate-x'] ?? 0, -100000, 100000)}px`);
  element.style.setProperty('--ubi-motion-y', `${clamp(channels['translate-y'] ?? 0, -100000, 100000)}px`);
  element.style.setProperty('--ubi-motion-scale', String(clamp(channels.scale ?? 1, 0.25, 2)));
  if (channels['switch-position'] !== undefined) element.style.setProperty('--ubi-switch-progress', String(clamp(channels['switch-position'], 0, 1)));
  if (channels.progress !== undefined) entry.progressFill?.style.setProperty('--ubi-progress-visual', `${clamp(channels.progress, 0, 100)}%`);
  if (channels.number !== undefined) {
    const visual = entry.numberVisual;
    const value = formatTicker(channels.number);
    if (visual && visual.textContent !== value) visual.textContent = value;
  }
  if (channels['disclosure-height'] !== undefined) {
    element.style.overflow = 'hidden';
    element.style.height = `${Math.max(0, channels['disclosure-height'])}px`;
  }
}

function tabindexables(element) {
  return [...element.querySelectorAll('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),a[href],[tabindex]:not([tabindex="-1"])')]
    .filter(item => !item.hidden && item.getClientRects().length > 0 && !item.closest('[inert]'));
}

/** Mount-owned finite presentation, reduced-motion preference and frame scheduling. */
export function createPresentation(root, options = {}) {
  if (!root || root.nodeType !== 1) throw new TypeError('Ubi presentation requires an element');
  const clock = getClock(root, options.clock);
  let mode = options.motion ?? 'system';
  if (!motionModes.has(mode)) throw new TypeError('Unsupported Ubi motion mode');
  const view = root.ownerDocument.defaultView;
  const document = root.ownerDocument;
  const reducedMedia = view.matchMedia?.('(prefers-reduced-motion: reduce)');
  const state = new MotionState();
  let frameId = null, disposed = false, hidden = document.visibilityState === 'hidden';
  let currentView = null, currentByKey = new Map(), currentEntries = new Map(), currentTargets = new Map();
  let currentFrame = { channels: new Map(), exits: [], active: false, nextWakeMs: null };
  const exitLayers = new Map(), indicators = new Map(), disclosureLayers = new Map();

  function isReduced() { return mode === 'reduced' || (mode === 'system' && Boolean(reducedMedia?.matches)); }
  function cancelFrame() { if (frameId !== null) { clock.cancelFrame(frameId); frameId = null; } }
  function schedule() { if (!disposed && !hidden && !isReduced() && frameId === null && currentFrame.active) frameId = clock.requestFrame(onFrame); }

  function applyFrame(frame = currentFrame, entries = currentEntries) {
    for (const [key, channels] of frame.channels) {
      const indicator = indicators.get(key);
      if (indicator) {
        indicator.style.width = `${Math.max(0, channels['indicator-width'] ?? 0)}px`;
        indicator.style.height = `${Math.max(0, channels['indicator-height'] ?? 0)}px`;
        indicator.style.transform = `translate3d(${channels['indicator-x'] ?? 0}px,${channels['indicator-y'] ?? 0}px,0)`;
        indicator.hidden = !(channels['indicator-width'] > 0 && channels['indicator-height'] > 0);
      }
      const entry = entries.get(key);
      if (entry) {
        setMotionStyle(entry, channels);
        const node = currentByKey.get(key);
        if (node?.props.Disclosure !== undefined) {
          entry.element.inert = !node.props.Disclosure;
          if (node.props.Disclosure) entry.element.removeAttribute('aria-hidden');
          else entry.element.setAttribute('aria-hidden', 'true');
          const layer = disclosureLayers.get(key);
          if (node.props.Disclosure || channels['disclosure-height'] <= 0) {
            layer?.remove(); disclosureLayers.delete(key);
          }
          if (node.props.Disclosure && !frame.active) { entry.element.style.removeProperty('height'); entry.element.style.removeProperty('overflow'); }
        }
      }
    }
    const liveExitKeys = new Set(frame.exits.map(exit => exit.key));
    for (const [key, record] of exitLayers) if (!liveExitKeys.has(key)) { record.layer.remove(); exitLayers.delete(key); }
    for (const exit of frame.exits) {
      const record = exitLayers.get(exit.key);
      if (!record) continue;
      const { base, layer, clone } = record;
      clone.style.setProperty('--ubi-motion-opacity', String(clamp(exit.channels.opacity ?? base.opacity, 0, 1)));
      const scale = (exit.channels.scale ?? base.scale) / (base.scale || 1);
      const x = (exit.channels['translate-x'] ?? base['translate-x']) - base['translate-x'];
      const y = (exit.channels['translate-y'] ?? base['translate-y']) - base['translate-y'];
      layer.style.transform = `translate3d(${x}px,${y}px,0) scale(${clamp(scale, 0.25, 2)})`;
    }
    currentFrame = frame;
  }

  function onFrame() {
    frameId = null;
    if (disposed || hidden) return;
    const frame = state.sample(clock.now(), isReduced());
    applyFrame(frame);
    schedule();
  }

  function collectIndicatorTargets(nextView, entries) {
    const byKey = new Map(nextView.nodes.map(node => [node.key, node]));
    const targets = new Map();
    const isSegmented = parent => {
      if (parent.kind !== 'Row' || !parent.children.length) return false;
      const items = parent.children.map(key => byKey.get(key));
      return items.every(node => node?.kind === 'ToggleItem' && node.props.Group === parent.key);
    };
    const parents = nextView.nodes.filter(node => node.props.Role === 'tablist' || isSegmented(node));
    for (const parent of parents) {
      const segmented = isSegmented(parent);
      const tabs = parent.children.map(key => byKey.get(key)).filter(node => node?.kind === (segmented ? 'ToggleItem' : 'Tab'));
      const selectedTab = tabs.find(node => node.props.Checked || node.props.Selected);
      const key = parent.key;
      const indicatorChannels = { 'indicator-x': 0, 'indicator-y': 0, 'indicator-width': 0, 'indicator-height': 0 };
      const container = entries.get(parent.key)?.element;
      if (container) container.classList.add(segmented ? 'ubi-segmented' : 'ubi-tablist');
      if (!selectedTab) {
        const indicator = indicators.get(parent.key);
        if (indicator) indicator.hidden = true;
        targets.set(key, { channels: indicatorChannels, motion: parent.props.Motion ?? selection });
        continue;
      }
      const tab = entries.get(selectedTab.key)?.element;
      if (!container || !tab) continue;
      let indicator = indicators.get(parent.key);
      if (!indicator) {
        indicator = document.createElement('span');
        indicator.className = 'ubi-tab-indicator';
        indicator.setAttribute('aria-hidden', 'true');
        indicators.set(parent.key, indicator);
      }
      if (indicator.parentElement !== container) container.prepend(indicator);
      const containerRect = container.getBoundingClientRect(), tabRect = tab.getBoundingClientRect();
      const motion = parent.props.Motion ?? selection;
      targets.set(key, {
        channels: {
          'indicator-x': tabRect.left - containerRect.left + container.scrollLeft,
          'indicator-y': tabRect.top - containerRect.top + container.scrollTop,
          'indicator-width': tabRect.width, 'indicator-height': tabRect.height,
        }, motion,
      });
    }
    for (const [key, indicator] of indicators) if (!parents.some(node => node.key === key)) {
      indicator.remove(); indicators.delete(key);
    }
    return targets;
  }

  function prepare(nextView, entries = currentEntries) {
    if (disposed) throw new TypeError('Ubi presentation is disposed');
    if (!nextView || !Array.isArray(nextView.nodes) || nextView.nodes.length > 4096) throw new TypeError('Invalid presentation view');
    const targets = normalizeTargets(makeTargets(nextView));
    const targetKeys = new Set(targets.map(target => target.key));
    const nowMs = clock.now();
    if (!Number.isFinite(nowMs) || nowMs < 0) throw new TypeError('Invalid Ubi host clock time');
    const removed = new Map();
    const nextByKey = new Map(nextView.nodes.map(node => [node.key, node]));
    const previousParents = new Map();
    for (const parent of currentView?.nodes ?? []) for (const child of parent.children) previousParents.set(child,parent.key);
    const suppressed = new Set();
    for (const [key,target] of currentTargets) {
      if (targetKeys.has(key)) continue;
      let parentKey=previousParents.get(key);
      while(parentKey) {
        const previousParent=currentByKey.get(parentKey),nextParent=nextByKey.get(parentKey);
        if ((nextParent && nextParent.props.Disclosure===false) || (!nextParent && previousParent && currentTargets.get(parentKey)?.exit!=='None')) {
          suppressed.add(key); break;
        }
        parentKey=previousParents.get(parentKey);
      }
    }
    const disclosureSnapshots = new Map();
    for (const node of nextView.nodes) {
      if (node.props.Disclosure!==false || currentByKey.get(node.key)?.props.Disclosure===false) continue;
      const element=entries.get(node.key)?.element;
      if (!element?.childNodes.length) continue;
      const layer=document.createElement('div'); layer.className='ubi-disclosure-exit'; layer.setAttribute('aria-hidden','true'); layer.inert=true;
      for (const child of [...element.childNodes]) {
        if (child.nodeType===1 && child.matches('.ubi-tab-indicator,.ubi-exit-layer,.ubi-disclosure-exit')) continue;
        const clone=child.cloneNode(true);
        if (clone.nodeType===1) stripSnapshotClone(clone);
        layer.append(clone);
      }
      disclosureSnapshots.set(node.key,layer);
    }
    for (const [key, target] of currentTargets) {
      if (targetKeys.has(key)) continue;
      const record = createExitLayer(root, entries.get(key), currentFrame.channels.get(key));
      if (record) removed.set(key, record);
    }
    return { owner: api, nextView, entries: null, targets, removed, suppressed:[...suppressed], disclosureSnapshots, nowMs, used: false };
  }

  function reconcileExits(frame, prepared) {
    const active = new Set(frame.exits.map(exit => exit.key));
    for (const [key, record] of exitLayers) if (!active.has(key)) { record.layer.remove(); exitLayers.delete(key); }
    for (const key of active) {
      if (exitLayers.has(key)) continue;
      const record = prepared.removed.get(key);
      if (!record) continue;
      exitLayers.set(key, record);
      root.append(record.layer);
    }
  }

  function commit(prepared, entries = prepared?.entries) {
    if (disposed) throw new TypeError('Ubi presentation is disposed');
    if (!prepared || prepared.owner !== api || prepared.used) throw new TypeError('Invalid prepared presentation');
    const nextEntries = entries ?? new Map();
    const indicatorTargets = collectIndicatorTargets(prepared.nextView, nextEntries);
    const targets = prepared.targets.map(target => {
      const indicator = indicatorTargets.get(target.key);
      if (!indicator) return target;
      const node = prepared.nextView.nodes.find(item => item.key === target.key);
      return {
        ...target,
        channels: { ...target.channels, ...indicator.channels },
        motion: node?.props.Motion ?? indicator.motion,
      };
    });
    for (const target of targets) {
      const node = prepared.nextView.nodes.find(item => item.key === target.key);
      const entry = nextEntries.get(target.key);
      if (node?.props.Disclosure !== undefined) target.channels['disclosure-height'] = node.props.Disclosure ? (entry?.element.scrollHeight ?? 0) : 0;
    }
    state.commit(targets, prepared.nowMs, isReduced(),prepared.suppressed);
    prepared.used = true;
    prepared.entries = nextEntries;
    currentView = prepared.nextView;
    currentByKey = new Map(prepared.nextView.nodes.map(node=>[node.key,node]));
    currentEntries = nextEntries;
    currentTargets = new Map(targets.map(target => [target.key, target]));
    const frame = state.sample(prepared.nowMs, isReduced());
    reconcileExits(frame, prepared);
    for (const [key,layer] of prepared.disclosureSnapshots) {
      const element=nextEntries.get(key)?.element;
      if (element) { element.append(layer); disclosureLayers.set(key,layer); }
    }
    applyFrame(frame, nextEntries);
    if (hidden) cancelFrame(); else schedule();
  }

  function sampleAt(nowMs) {
    if (disposed) return { channels: new Map(), exits: [], active: false, nextWakeMs: null };
    const frame = state.sample(nowMs, isReduced());
    applyFrame(frame);
    if (hidden || isReduced()) cancelFrame(); else schedule();
    return frame;
  }

  function setMotion(nextMode) {
    if (!motionModes.has(nextMode)) throw new TypeError('Unsupported Ubi motion mode');
    if (disposed) return;
    const wasReduced = isReduced(); mode = nextMode;
    const nowReduced = isReduced();
    if (nowReduced) {
      cancelFrame();
      const frame = state.sample(clock.now(), true);
      applyFrame(frame);
    } else if (wasReduced !== nowReduced) {
      const frame = state.sample(clock.now(), false);
      applyFrame(frame); schedule();
    }
  }

  function onPreference() { if (mode === 'system') setMotion('system'); }
  function onVisibility() {
    hidden = document.visibilityState === 'hidden';
    if (hidden) cancelFrame();
    else { const frame = state.sample(clock.now(), isReduced()); applyFrame(frame); schedule(); }
  }

  const api = {
    prepare,
    commit,
    sample: sampleAt,
    setMotion,
    dispose() {
      if (disposed) return;
      disposed = true; cancelFrame();
      reducedMedia?.removeEventListener?.('change', onPreference);
      document.removeEventListener('visibilitychange', onVisibility);
      for (const record of exitLayers.values()) record.layer.remove();
      exitLayers.clear();
      for (const layer of disclosureLayers.values()) layer.remove();
      disclosureLayers.clear();
      for (const indicator of indicators.values()) indicator.remove();
      indicators.clear(); currentEntries.clear(); currentTargets.clear(); currentByKey.clear(); currentView = null;
      state.dispose();
    },
  };

  reducedMedia?.addEventListener?.('change', onPreference);
  document.addEventListener('visibilitychange', onVisibility);
  return api;
}

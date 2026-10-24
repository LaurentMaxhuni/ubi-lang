const easings = new Set(['Linear', 'EaseOut', 'EaseInOut', 'Spring']);
const transitions = new Set(['None', 'Fade', 'Rise', 'Scale']);
const editableKinds = new Set(['TextInput', 'PasswordInput', 'TextArea', 'NumberInput']);
const visualProps = new Set([
  'Content', 'Label', 'Tone', 'Variant', 'Checked', 'Disabled', 'Selected', 'Pressed', 'Expanded',
  'Width', 'Height', 'WidthPercent', 'HeightPercent', 'MinWidth', 'MinHeight', 'MaxWidth', 'MaxHeight',
  'Padding', 'Gap', 'Grow', 'Columns', 'FontSize', 'Center', 'Clip', 'Progress', 'Level', 'Role',
  'Opacity', 'Translate', 'Scale', 'Effect', 'Number', 'OverlayKind', 'Open', 'Placement', 'TimeoutMs',
  'Option', 'Group', 'Index',
]);
const motionKeys = ['durationMs', 'delayMs', 'easing'];
const clamp = (value, low, high) => Math.min(high, Math.max(low, value));
const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value);

function validateMotion(motion) {
  if (!isRecord(motion)) throw new TypeError('Invalid Ubi motion timing');
  const safe = copyDataRecord(motion, 'motion timing');
  if (Object.keys(safe).length !== motionKeys.length || motionKeys.some(key => !Object.hasOwn(safe, key)) ||
      !Number.isInteger(safe.durationMs) || safe.durationMs < 0 || safe.durationMs > 2000 ||
      !Number.isInteger(safe.delayMs) || safe.delayMs < 0 || safe.delayMs > 1000 || !easings.has(safe.easing)) {
    throw new TypeError('Invalid Ubi motion timing');
  }
  return { durationMs: safe.durationMs, delayMs: safe.delayMs, easing: safe.easing };
}

function validateTransition(transition) {
  if (transition !== undefined && !transitions.has(transition)) throw new TypeError('Invalid Ubi transition');
  return transition ?? 'None';
}

function validateTime(time, name) {
  if (!Number.isFinite(time) || time < 0) throw new TypeError(`Invalid ${name}`);
}

function clampChannel(name, value) {
  if (name === 'opacity') return clamp(value, 0, 1);
  if (name === 'scale') return clamp(value, 0.25, 2);
  if (name === 'translate-x' || name === 'translate-y') return clamp(value, -100000, 100000);
  return value;
}

function validChannel(name, value, sampled = false) {
  if (!Number.isFinite(value)) return false;
  if (name === 'opacity') return value >= 0 && value <= 1;
  if (name === 'scale') return value >= 0.25 && value <= 2;
  if (name === 'translate-x' || name === 'translate-y' || name === 'indicator-x' || name === 'indicator-y') return Math.abs(value) <= 100000;
  if (name === 'switch-position') return value >= 0 && value <= 1;
  if (name === 'progress') return value >= 0 && value <= 100;
  if (name === 'number') return (sampled || Number.isInteger(value)) && value >= -2147483648 && value <= 2147483647;
  if (name === 'disclosure-height' || name === 'indicator-width' || name === 'indicator-height') return value >= 0 && value <= 100000;
  return Math.abs(value) <= 100000;
}

function ease(t, easing) {
  switch (easing) {
    case 'Linear': return t;
    case 'EaseOut': return 1 - (1 - t) ** 3;
    case 'EaseInOut': return t < 0.5 ? 4 * t ** 3 : 1 - ((-2 * t + 2) ** 3) / 2;
    case 'Spring': {
      const curve = x => 1 - Math.exp(-6 * x) * (Math.cos(10 * x) + 0.6 * Math.sin(10 * x));
      return curve(t) / curve(1);
    }
    default: throw new TypeError('Invalid Ubi easing');
  }
}

/** Sample one bounded scalar track. A zero duration applies its target immediately. */
export function sample(from, to, startMs, nowMs, motion) {
  if (!Number.isFinite(from) || !Number.isFinite(to)) throw new TypeError('Motion values must be finite');
  validateTime(startMs, 'motion start time');
  validateTime(nowMs, 'motion sample time');
  const timing = validateMotion(motion);
  if (timing.durationMs === 0) return to;
  const elapsed = nowMs - startMs - timing.delayMs;
  if (elapsed <= 0) return from;
  if (elapsed >= timing.durationMs) return to;
  const progress = elapsed / timing.durationMs;
  return from + (to - from) * ease(progress, timing.easing);
}

function copyDataRecord(value, name) {
  if (!isRecord(value)) throw new TypeError(`Invalid ${name}`);
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const result = {};
  for (const [key, descriptor] of Object.entries(descriptors)) {
    if (!Object.hasOwn(descriptor, 'value') || !descriptor.enumerable) throw new TypeError(`Invalid ${name}`);
    result[key] = descriptor.value;
  }
  return result;
}

function sanitizeSnapshot(node, depth = 0, seen = new Set(), budget = { count: 0 }) {
  const raw = copyDataRecord(node, 'visual snapshot');
  if (depth > 32 || typeof raw.key !== 'string' || typeof raw.kind !== 'string') throw new TypeError('Invalid visual snapshot');
  if (editableKinds.has(raw.kind)) return null;
  if (++budget.count > 4096) throw new TypeError('Visual snapshot exceeds mount limit');
  if (seen.has(raw.key)) throw new TypeError('Cyclic visual snapshot');
  seen.add(raw.key);
  const props = copyDataRecord(raw.props ?? {}, 'visual snapshot properties');
  const style = copyDataRecord(raw.style ?? {}, 'visual snapshot style');
  const safeProps = {};
  for (const key of Object.keys(props)) if (visualProps.has(key)) safeProps[key] = props[key];
  const safeStyle = {};
  for (const [key, value] of Object.entries(style)) {
    if (['gap', 'padding', 'fontSize', 'columns', 'width', 'height'].includes(key)) safeStyle[key] = value;
  }
  const children = raw.children ?? [];
  if (!Array.isArray(children)) throw new TypeError('Invalid visual snapshot children');
  const safeChildren = [];
  for (const child of children) {
    const safe = sanitizeSnapshot(child, depth + 1, seen, budget);
    if (safe) safeChildren.push(safe);
  }
  seen.delete(raw.key);
  return { key: raw.key, kind: raw.kind, props: safeProps, style: safeStyle, children: safeChildren };
}

function normalizeTarget(target) {
  const raw = copyDataRecord(target, 'motion target');
  if (typeof raw.key !== 'string' || !raw.key || typeof raw.kind !== 'string') throw new TypeError('Invalid motion target identity');
  const channels = copyDataRecord(raw.channels, 'motion channels');
  const names = Object.keys(channels);
  if (names.length > 16 || names.some(name => !/^[a-z][a-z0-9-]{0,63}$/.test(name)) || names.some(name => !validChannel(name, channels[name]))) {
    throw new TypeError('Invalid motion target channels');
  }
  const normalizedChannels = { opacity: 1, 'translate-x': 0, 'translate-y': 0, scale: 1, ...channels };
  if (Object.keys(normalizedChannels).length > 16) throw new TypeError('Invalid motion target channel count');
  const motion = raw.motion ?? { durationMs: 0, delayMs: 0, easing: 'EaseOut' };
  const timing = validateMotion(motion);
  const enter = validateTransition(raw.enter), exit = validateTransition(raw.exit);
  const snapshot = raw.snapshot == null ? null : sanitizeSnapshot(raw.snapshot);
  return {
    key: raw.key, kind: raw.kind,
    channels: normalizedChannels,
    motion: timing, enter, exit, snapshot,
  };
}

export function normalizeTargets(targets) {
  if (!Array.isArray(targets) || targets.length > 4096) throw new TypeError('Invalid motion target list');
  const prepared = targets.map(normalizeTarget);
  const keys = new Set();
  for (const target of prepared) {
    if (keys.has(target.key)) throw new TypeError('Duplicate motion target key');
    keys.add(target.key);
  }
  return prepared;
}

function transitionStart(channels, transition) {
  const start = { ...channels };
  if (transition === 'Fade' || transition === 'Rise' || transition === 'Scale') start.opacity = 0;
  if (transition === 'Rise') start['translate-y'] = (start['translate-y'] ?? 0) + 12;
  if (transition === 'Scale') start.scale = (start.scale ?? 1) * 0.96;
  for (const [name,value] of Object.entries(start)) start[name]=clampChannel(name,value);
  return start;
}

function transitionEnd(channels, transition) {
  const end = { ...channels };
  if (transition === 'Fade' || transition === 'Rise' || transition === 'Scale') end.opacity = 0;
  if (transition === 'Rise') end['translate-y'] = (end['translate-y'] ?? 0) + 12;
  if (transition === 'Scale') end.scale = (end.scale ?? 1) * 0.96;
  for (const [name,value] of Object.entries(end)) end[name]=clampChannel(name,value);
  return end;
}

function makeTracks(from, to, startMs, motion) {
  const tracks = new Map();
  const values = {};
  for (const name of new Set([...Object.keys(from), ...Object.keys(to)])) {
    const fallback = name === 'opacity' || name === 'scale' ? 1 : 0;
    const source = Object.hasOwn(from, name) ? from[name] : fallback;
    const target = Object.hasOwn(to, name) ? to[name] : source;
    if (!validChannel(name, source, true) || !validChannel(name, target)) throw new TypeError('Invalid Ubi motion channel value');
    if (motion.durationMs !== 0 && source !== target) {
      tracks.set(name, { from: source, to: target, startMs, motion });
      values[name] = source;
    } else values[name] = target;
  }
  return { values, tracks };
}

function sampleTracks(values, tracks, nowMs) {
  const current = { ...values };
  for (const [name, track] of tracks) current[name] = clampChannel(name, sample(track.from, track.to, track.startMs, nowMs, track.motion));
  return current;
}

function trackDeadline(track) {
  return track.motion.durationMs === 0 ? track.startMs : track.startMs + track.motion.delayMs + track.motion.durationMs;
}

/** Keyed finite presentation state. It contains no DOM, clock, or application reducer. */
export class MotionState {
  #nodes = new Map();
  #exits = new Map();
  #disposed = false;

  commit(targets, nowMs, reduced = false, suppressExitKeys = []) {
    if (this.#disposed) throw new TypeError('Motion state is disposed');
    validateTime(nowMs, 'motion commit time');
    const prepared = normalizeTargets(targets);
    if (!Array.isArray(suppressExitKeys) || suppressExitKeys.length > 4096 || suppressExitKeys.some(key => typeof key !== 'string')) throw new TypeError('Invalid suppressed exit list');
    const suppressed = new Set(suppressExitKeys);
    const keys = new Set(prepared.map(target => target.key));

    const current = this.sample(nowMs, false);
    const live = current.channels;
    const exiting = new Map(current.exits.map(exit => [exit.key, exit]));
    const nextNodes = new Map(), nextExits = new Map();
    for (const target of prepared) {
      const old = this.#nodes.get(target.key);
      const exit = exiting.get(target.key);
      const sameOld = old?.kind === target.kind ? live.get(target.key) : null;
      const sameExit = exit?.kind === target.kind ? exit.channels : null;
      const from = sameOld ?? sameExit ?? (target.enter === 'None' ? target.channels : transitionStart(target.channels, target.enter));
      const tracks = makeTracks(from, target.channels, nowMs, target.motion);
      if (reduced) { tracks.values && Object.assign(tracks.values, target.channels); tracks.tracks.clear(); }
      nextNodes.set(target.key, {
        kind: target.kind, values: tracks.values, tracks: tracks.tracks,
        motion: target.motion, exit: target.exit, snapshot: target.snapshot,
      });
    }

    for (const [key, old] of this.#nodes) {
      if (keys.has(key) || suppressed.has(key)) continue;
      const visible = live.get(key);
      const nextTransition = old.exit;
      if (!reduced && nextTransition !== 'None' && old.snapshot && visible) {
        const to = transitionEnd(visible, nextTransition);
        const tracks = makeTracks(visible, to, nowMs, old.motion);
        if (tracks.tracks.size) nextExits.set(key, {
          key, kind: old.kind, values: tracks.values, tracks: tracks.tracks,
          snapshot: old.snapshot, order: nowMs + nextExits.size / 1e9,
        });
      }
    }
    for (const [key, exit] of this.#exits) {
      if (!keys.has(key) && !this.#nodes.has(key) && !exiting.has(key)) continue;
      if (!keys.has(key) && !nextExits.has(key) && exiting.has(key)) nextExits.set(key, {
        key: exit.key, kind: exit.kind, values: exit.values, tracks: exit.tracks,
        snapshot: exit.snapshot, order: exit.order,
      });
    }

    if (reduced) nextExits.clear();
    const snapshotCount = snapshot => snapshot ? 1 + snapshot.children.reduce((count, child) => count + snapshotCount(child), 0) : 0;
    while ([...nextExits.values()].reduce((count, exit) => count + snapshotCount(exit.snapshot), 0) > 4096) {
      const oldest = [...nextExits.values()].sort((a, b) => a.order - b.order)[0];
      nextExits.delete(oldest.key);
    }
    this.#nodes = nextNodes;
    this.#exits = nextExits;
  }

  sample(nowMs, reduced = false) {
    validateTime(nowMs, 'motion sample time');
    if (this.#disposed) return { channels: new Map(), exits: [], active: false, nextWakeMs: null };
    const channels = new Map();
    let nextWakeMs = null, active = false;
    for (const [key, node] of this.#nodes) {
      const values = reduced ? { ...node.values } : sampleTracks(node.values, node.tracks, nowMs);
      for (const [name, track] of [...node.tracks]) {
        if (reduced || nowMs >= trackDeadline(track)) { values[name] = clampChannel(name, track.to); node.tracks.delete(name); }
        else { active = true; const deadline = trackDeadline(track); nextWakeMs = nextWakeMs === null ? deadline : Math.min(nextWakeMs, deadline); }
      }
      if (reduced) node.tracks.clear();
      node.values = values;
      channels.set(key, { ...values });
    }

    const exits = [];
    for (const [key, exit] of [...this.#exits]) {
      if (reduced) { this.#exits.delete(key); continue; }
      const values = sampleTracks(exit.values, exit.tracks, nowMs);
      let pending = false;
      for (const [name, track] of [...exit.tracks]) {
        if (nowMs >= trackDeadline(track)) { values[name] = clampChannel(name, track.to); exit.tracks.delete(name); }
        else { pending = true; const deadline = trackDeadline(track); nextWakeMs = nextWakeMs === null ? deadline : Math.min(nextWakeMs, deadline); }
      }
      exit.values = values;
      if (pending) { active = true; exits.push({ key, kind: exit.kind, channels: { ...values }, snapshot: exit.snapshot }); }
      else this.#exits.delete(key);
    }
    if (reduced) { active = false; nextWakeMs = null; }
    return { channels, exits, active, nextWakeMs };
  }

  dispose() {
    this.#nodes.clear(); this.#exits.clear(); this.#disposed = true;
  }
}

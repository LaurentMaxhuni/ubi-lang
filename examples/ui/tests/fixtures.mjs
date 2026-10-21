export const e = (tag, ...values) => ({ tag, values });
export const node = (key, kind = 'Column', props = [], children = []) => ({ key, kind: e(kind), props, children });
export const arena = (nodes = [node('root')], root = nodes[0].key) => ({ schemaVersion: 1, root, nodes });
export const button = (key, message = e('Increment')) => node(key, 'Button', [
  e('Content', 'Add one'), e('Variant', e('Primary')), e('Disabled', false),
  e('Busy', false), e('Activate', message),
]);

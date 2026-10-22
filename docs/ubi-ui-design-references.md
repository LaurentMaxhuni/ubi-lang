# Ubi UI visual references

The Kombai web gallery informed the homepage composition:

- [Oriel](https://kombai.com/gallery/inspirations/technology-websites-web-56/): spacious, asymmetric introduction with a visual beside the text.
- [Quorum](https://kombai.com/gallery/inspirations/technology-websites-web-57/): restrained sage surfaces and tactile layering.
- [Halden](https://kombai.com/gallery/inspirations/widgets-web-54/): compact, useful widgets with clear controls.

Ubi uses its own warm paper, forest ink, serif headings, and source workbench. The hero runs the actual Tabs Ubi example, with live interaction and highlighted source. The full gallery follows below; documentation pages retain a quieter reading layout. No reference imagery or implementation was copied.

## Animated component research, 2026-10-22

User requested a substantially larger component library with beautiful animation,
starting with research into Skiper and similar libraries. This is a reference
report and proposed scope, not an approved implementation specification.

### References

Reviewed official catalogs/documentation. Skiper and Animate UI were also opened
in the browser; this research does not establish their performance or accessibility.

| Reference | Examples found | Proposed use in Ubi |
| --- | --- | --- |
| [Skiper UI](https://skiper-ui.com/) | Dynamic island, image reveal, hover members, drag/scroll interactions, tooltips | Expressive interaction details and tactile feedback |
| [React Bits](https://reactbits.dev/get-started/index) | Split Text, Blur Text, Rotating Text, Tilted Card, Animated List, Magnet, backgrounds | Text entrances, interactive cards, selectively used visual effects |
| [Aceternity UI](https://ui.aceternity.com/components) | Wobble Card, Card Spotlight, Floating Dock, Animated Modal, Sticky Scroll Reveal | Interactive cards, navigation, overlays and composed sections |
| [Magic UI](https://magicui.design/docs/components) | Number Ticker, Border Beam, Shimmer Button, Marquee, Blur Fade | Compact reusable effects and animated data feedback |
| [Animate UI](https://animate-ui.com/) | Animated switches, tabs, accordion, checkbox, sliding numbers, progress | Motion on everyday controls, retaining meaningful control states |
| [Motion Primitives](https://motion-primitives.com/docs/text-effect) | Per-character and per-word text effects, presets and delay controls | Small composable animation primitives |
| [Kokonut UI](https://kokonutui.com/docs) | Notification Stack, Smooth Drawer, Command Palette, Status Island, Hold Button, Save Button | Product interactions, notification feedback and stateful buttons |
| [Cult UI](https://www.cult-ui.com/) | Dynamic Island, Shift Card, Folded Card, collaboration toolbar, Kanban board | Richer widgets, expandable surfaces and card transitions |
| [UI Layouts](https://www.ui-layouts.com/components) | Stacking Card, Motion Drawer, Image Reveal, Motion Number, Progressive Carousel | Layout transitions, drawers and gallery composition |

Motion Primitives blocked direct page retrieval with HTTP 403; its Text Effect
documentation was available through the search index. That reference has lower
verification depth than the directly retrieved catalogs. Skiper, Aceternity,
Kokonut and other catalogs contain paid offerings alongside public examples;
listing a reference does not imply all its components are free or reusable.

### Recommendation

Combine Skiper's expressive interactions with Animate UI's everyday control
motion, then add selected React Bits / Magic UI effects. Preserve Ubi's current
identity. References guide behavior and visual direction; proposed components
would be original Ubi implementations, not React wrappers or copied paid source.

The checked catalog currently contains 40 components. The Ubi `Prop` contract has
no motion properties, and the theming guide explicitly records animated state
transitions as unimplemented. Existing Tabs, Accordion, Switch, Progress and
Skeleton should gain real motion rather than being counted as new components.

### Proposed expansion: 30 additional components

**15 application components:** Dialog, AlertDialog, Drawer, Popover, Tooltip,
Toast, Select, Combobox, DropdownMenu, ContextMenu, CommandPalette, Pagination,
Stepper, Timeline and Table.

**15 expressive components:** SpotlightCard, TiltCard, CardStack, Carousel, Dock,
Marquee, NumberTicker, TextReveal, RotatingText, ShimmerButton, BorderBeam,
AuroraBackground, GridBackground, ScrollReveal and AnimatedList.

These are candidates, not existing exports. Component names and contracts need
design review. Pointer effects require useful touch/keyboard behavior; image,
shader, scroll and 3D effects depend on renderer capabilities and may need a
later slice. A status island is a promising follow-up once overlays and shared
layout transitions work.

### Proposed sequence and acceptance criteria

1. Define a small shared motion contract for keyed Ubi views: timing, easing,
   interruption, entrance/exit lifetime and reduced-motion behavior. Keep
   application state deterministic; hosts drive presentation frames.
2. Prove it on existing controls: sliding tab/segmented indicators, accordion
   expansion, switch movement, progress interpolation and skeleton shimmer.
3. Add overlays and selection components with focus management, dismissal,
   keyboard navigation and source-owned Ubi examples.
4. Add expressive components in bounded groups; distinguish shared behavior
   from any effects initially supported by only one renderer.

Every delivered component needs a real interactive documentation preview,
copyable Ubi source, light/dark styling and narrow-screen behavior. Verify
interrupted/repeated input, reduced motion, focus retention and final state;
native GPU stills cannot establish animation correctness. Report platform
coverage only for hosts actually exercised. Avoid perpetual redraw when idle,
and pause optional ambient effects when hidden or motion is reduced.

# ADR 0018: Embedded branded Home

Date: 2026-09-08. Status: implementation authorized by the user's request to
replace the main menu with the separately reviewed bitmap design.

Reuse the portrait, wordmark and composition from the sibling
`branding-main-menu/bitmap` study. Embed PNG assets in the production executable;
no Python runtime or sidecar artwork is required. Ratatui-image uses the actual
terminal graphics and cell-size query once, after alternate-screen entry and
before ordinary event reading. Failed/unsupported detection falls back to the
existing functional text menu. Do not force Sixel on an unverified terminal.

The bitmap composition requires 100x36 and the Ash true-color presentation.
Smaller supported terminals, NO_COLOR and non-graphics hosts
use a compact branded text Home down to 40x20. Settings/Help require 80x24
and provide an Esc return to Home; gameplay retains its own minimum. The bitmap palette bypasses table
palette remapping and cell animation. Cache encoded images by viewport and redraw
Home only after input or resize; clear images before leaving Home and when a
synchronous game/entry flow returns.

ShellApp remains the navigation authority. All six menu entries come from shared
HOME_ITEMS; Settings uses the existing editor/store and Study stays unavailable.
Practice, Host and Join invoke their existing production flows. No poker engine,
network version, credentials or profile format changes are introduced.

The standalone preview is preserved as design provenance, not a runtime dependency.
Full public release, packaging changes, and new Study behavior are outside scope.

2026-09-09: One production appearance; legacy saved theme/motion fields are read
for compatibility but no longer choose presentation. Settings exposes player
name and Practice stack only. Automatic terminal capability fallbacks remain.

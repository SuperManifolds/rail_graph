# Unreleased

## Bug Fixes
- Fixed changelog window not expanding when resized (was maintaining fixed internal height)
- Fixed help text in train number format to show correct single brace syntax
- Fixed time inputs displaying in 12-hour format for some users (now always shows 24-hour format)

## Improvements
- Line thickness slider now adjusts in increments of 0.25
- Replaced custom markdown parser with pulldown-cmark for proper CommonMark support in changelog
- Added proper styling for tables, code blocks, links, and blockquotes in changelog display
- Increased train number format input width for better visibility
- Added separate 'Return Last Departure' field for return journeys in auto schedule mode

# v0.1.2 - 2025-10-22

### Bug Fixes

UI & Navigation
- Fixed nonexistent CSS file reference that caused browser errors
- Edit Line dialog now displays line name instead of UUID in title
- Clicking conflicts in the list now preserves user's horizontal zoom setting
- Removed redundant "showing all conflicts" status text from conflict list

Dialog Windows
- Dialog windows now constrain to viewport height

CSV Import
- First station now imports correctly even when arrival time is blank (falls back to departure time or zero)
- CSV column mapper sample values now refresh properly when selecting different files with same column count or re-selecting files

### Feature Enhancement

CSV Import Improvements
- Lines imported with specific arrival/departure times now use manual scheduling mode with a single departure at the specified time, providing more accurate representation of real timetables

# v0.1.1 - 2025-10-22

## Bug Fixes
- Fixed an issue that would cause the page to refresh every few seconds
- Fixed route direction toggle (Forward/Return) not updating displayed wait times
- Fixed time display mode toggle (Cumulative/Next-stop) not updating the UI
- Fixed WASD and spacebar keyboard shortcuts triggering while typing in input fields
- Fixed junction names appearing as empty entries in 'Connect to' dropdown

## Features
- Added 'Connect to' field now defaults to the most recently added station

# v0.1.0 - 2025-10-22

## What's Changed
* refactor: Rename application by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/22
* Refactor by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/23
* Tests by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/24
* Junctions by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/25
* Scheduling by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/26
* Performance optimizations by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/27
* Refactor CSS with variables and mixins for theming support by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/28
* Views by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/29
* feat: add multiple save files with project manager by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/30
* feat: implement project import/export (#21) by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/31
* Jtraingraph by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/32
* Misc by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/34
* feat: add distance-based vertical spacing for time graph (#8) by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/35
* refactor: only draw station dots when train has wait time by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/36
* Supermanifolds/infrastructure fixes by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/37
* feat: add AWS ECS deployment infrastructure by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/38
* feat: add HTTPS support to ALB by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/39
* Alex/docs by @SuperManifolds in https://github.com/SuperManifolds/rail_graph/pull/40

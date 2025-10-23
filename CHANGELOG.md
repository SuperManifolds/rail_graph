# Unreleased

## Bug Fixes
- Fixed changelog window not expanding when resized (was maintaining fixed internal height)
- Fixed help text in train number format to show correct single brace syntax
- Fixed time inputs displaying in 12-hour format for some users (now always shows 24-hour format)
- Fixed default wait time only being applied to first stop when creating new routes
- Fixed project timestamps displaying in UTC instead of user's local time
- Fixed "Save As" with an existing project name not prompting for overwrite confirmation (now shows a confirmation dialog)
- Fixed "Save As" not including recent changes (new lines, stations, etc.) made before opening the project dialog
- Fixed track editor distance field retaining value from previously edited track when switching between tracks
- Fixed station editor default platform selection resetting to "Auto" when trying to select a specific platform
- Fixed junction labels not being clickable
- Fixed junction names not appearing in track editor when editing tracks connected to junctions
- Fixed wait times not syncing correctly between forward and return routes when "Keep forward and return routes in sync" is enabled (now properly accounts for wait time shifts when reversing routes)
- Fixed "All days" button in days of week selector not updating checkbox states
- Fixed checkbox and help text layout in line editor to have proper spacing and alignment

## Improvements
- Infrastructure autolayout now snaps stations to a grid and intelligently preserves your manual positioning and branch orientations when adding or modifying the network
- Line thickness slider now adjusts in increments of 0.25
- Replaced custom markdown parser with pulldown-cmark for proper CommonMark support in changelog
- Added proper styling for tables, code blocks, links, and blockquotes in changelog display
- Increased train number format input width for better visibility
- Added separate 'Return Last Departure Before' field for return journeys in auto schedule mode
- Updated last departure labels to "Last Departure Before" for clarity
- Added configurable default wait time per line (used when creating new stops)
- First stop can now have a wait time (train departs after waiting at the first station)
- Time and duration inputs now support NIMBY Rails quick entry format (e.g., "45" for 45 seconds, "3.30" for 3 minutes 30 seconds, "5.15." for 5 hours 15 minutes, with support for . , : ; separators)
- Right-clicking stations, junctions, and tracks in infrastructure view now opens their editor (same as double-clicking)
- Track directions now automatically adjust when adding or removing tracks (1 track=bidirectional, 2 tracks=one each direction, 3+ tracks follow standard pattern with middle tracks bidirectional for odd counts). Lines automatically update their track assignments to remain compatible.
- Junction placement mode now automatically exits after placing a junction
- Any station can now be moved when editing a station (not just the one being edited)
- Increased train number input width in manual schedule mode and shortened placeholder text for better usability
- Station and junction labels in infrastructure view now maintain consistent orientation along each branch to minimize overlaps and remain stable while zooming
- Increased line name input width in line editor for better visibility

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

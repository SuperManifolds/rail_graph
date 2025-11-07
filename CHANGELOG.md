# v0.1.18 - 2025-11-05

## Features
- Platform and track editor windows now automatically resize horizontally when tracks or platforms are added or removed (resolves #98)
- Added canvas controls hint overlay that appears when opening infrastructure or graph views, displaying keyboard shortcuts for pan, zoom, and navigation controls with automatic dismissal on first interaction
- Added multi-select functionality for stations in infrastructure editor - drag to select multiple stations and use the floating toolbar to rotate (clockwise/counter-clockwise in 45° increments), align, bulk add/remove platforms, bulk add/remove tracks, or delete

## Improvements
- Significantly improved infrastructure editor performance when panning and zooming with large networks - viewport culling now skips rendering offscreen elements, and internal data structures are cached to reduce redundant calculations

## Bug Fixes
- Fixed station editor resetting unsaved field edits when changing other fields - form now only reloads when dialog opens, preserving all unsaved changes until Save is clicked
- Fixed project list not being scrollable when it overflows vertically
- Fixed Enter key not working to confirm in confirmation dialogs

# v0.1.17 - 2025-11-05

## Features
- Added automatic next-day rollover for departure times - when "Last Departure Before" is set to a time earlier than "First Departure" (e.g., First: 23:00, Last: 02:00), the system now automatically schedules trains into the next day with a "+1" indicator displayed in the time input field

# v0.1.16 - 2025-11-04

## Bug Fixes
- Fixed last stop in line editor stops list not displaying platform picker, wait time field, estimated time, or deletion button - the last stop now correctly renders all UI elements

# v0.1.15 - 2025-11-03

## Improvements
- Added informational text to line editor stops list explaining that empty travel time entries apply to all intermediate stops until the next time is specified
- Enhanced conflict detection to flag timing uncertainty when trains pass through stations with inherited duration timing - conflicts now display a warning indicating that exact timing is uncertain but must still be treated as real conflicts

## Bug Fixes
- Fixed CSV import pathfinding mode incorrectly distributing travel times across intermediate stations - the total travel time from the CSV now correctly applies to the entire path between stations using duration inheritance, allowing intermediate stations to be visited proportionally during the journey
- Fixed intermittent app crash when keyboard events fire after components are destroyed - keyboard shortcut handlers now safely handle disposed signals instead of panicking

# v0.1.14 - 2025-11-02

## Bug Fixes
- Fixed CSV import with Offset column format incorrectly calculating travel times by including wait time from the previous station - travel times now correctly represent actual time in motion between stations by subtracting wait time from the previous station's offset

# v0.1.13 - 2025-11-01

## Features
- Added departure offset lock feature in auto schedule settings - toggle the padlock button to maintain the time offset between forward and return departures. When locked, adjusting one departure time automatically shifts the corresponding return departure by the same amount, making it easy to maintain consistent scheduling patterns
- Added nested folder organisation of lines in the sidebar

## Improvements
- Reorganized auto schedule time fields - forward and return departures are now side-by-side for easier comparison
- Improved performance when making rapid edits - conflict detection now waits until you're done making changes before recalculating
- Journey continuation arrows in time-distance graphs now point horizontally to show the direction of travel
- Single-platform stations in time graph view now display with more muted colors to visually distinguish them from multi-platform stations that can support passing loops
- Passing stops (zero wait time) in line editor stops list now display at reduced opacity to emphasize actual stops where trains wait, improving visual hierarchy
- When a line editor is open, all journeys not belonging to that line are dimmed in the time graph, making it easier to focus on the line being edited

## Bug Fixes
- Fixed keyboard shortcuts (j/l) in time and duration inputs losing focus when used repeatedly in the stops list - inputs now maintain focus allowing for rapid adjustments
- Fixed infrastructure view station labels overlapping and aligning horizontally when zoomed out - labels now avoid positioning in the direction of connected tracks, resulting in better distribution and readability at all zoom levels
- Fixed stale values displaying in text inputs when switching between items in infrastructure view - track distance, station name, and junction name inputs now correctly update to show the current item's values instead of retaining previous values
- Fixed passing loop platform assignment when inserting station on existing track - both directions of lines now correctly use direction-based platform assignment instead of both being assigned to platform 0

# v0.1.12 - 2025-10-29

## Bug Fixes
- Fixed sync forward and return journey feature overwriting user-configured wait times in return route - wait times are now preserved when syncing routes

# v0.1.11 - 2025-10-29

## Features
- Added undo/redo functionality - use Cmd/Ctrl+Z to undo changes and Cmd/Ctrl+Shift+Z (or Ctrl+Y on Windows) to redo

## Improvements
- Time and duration inputs now support j/l keyboard shortcuts to quickly adjust values by 30 seconds (j decreases, l increases)
- Added keyboard shortcuts for horizontal scale adjustment: [ to decrease, ] to increase (adjusts time axis zoom in time-distance graphs)
- Train journey lines in time-distance graphs now show arrow indicators (↑ ↓) when routes extend beyond the visible station range, making it clear when journeys start before or continue after the filtered view

# v0.1.10 - 2025-10-28

## Bug Fixes
- Fixed false head-on conflicts on double-track railways caused by incorrect track assignment when the first edge of a route was traversed backward - track assignment now correctly determines edge traversal direction by examining connectivity between consecutive segments instead of assuming routes always start at the source of their first edge
- Fixed train journey rendering bug where final segments were not displayed on the graph when view filtering removed stations outside the visible path - view filtering now correctly filters both station_times and segments arrays to maintain data consistency

# v0.1.9 - 2025-10-28

## Improvements
- Station label section width is now resizable by dragging the edge
- Line controls sidebar width is now resizable by dragging the edge
- Return route travel times now display calculated values from forward route when sync is enabled
- Add Station dialog now supports canvas-based placement - click on canvas while dialog is open to set station position (with preview shown on canvas), click on empty space to place at that position (snapped to grid), or click on a track segment to insert station in the middle and automatically split the segment and update affected lines. Right-click preview station to clear position
- Station deletion now intelligently handles 2-connection stations by creating direct bypass connections - when deleting a station with exactly 2 connections, a new direct connection is automatically created between the neighboring stations with combined distance and the track configuration from the segment with more tracks. Lines passing through are automatically updated to use the new bypass path. Stations with more than 2 connections will break lines at that point (with appropriate warning in delete confirmation dialog)

## Bug Fixes
- Fixed sync forward and return routes feature to correctly calculate return journey times from forward route with proper inheritance of gap segments
- Fixed conflicts from lines outside the current view appearing when those lines shared stations with the view but used different routes

# v0.1.8 - 2025-10-27

## Performance
- Improved infrastructure view performance when zooming and panning on large networks

## Improvements
- Manual departure 'Until' field now only updates when finished editing instead of on every keystroke
- Added keyboard shortcut to reset view to default zoom and pan position (default: R key)
- Added About button to Settings that opens the changelog

## Bug Fixes
- Fixed train position dots and labels being obscured by the time scrubber line
- Fixed CSV import with arrival/departure time columns drifting away from input values
- Fixed manual departures disappearing when auto scheduling is enabled - manual departures now run alongside auto-scheduled services
- Fixed train journey lines in time-distance graph views connecting to wrong occurrence when a route visits the same junction multiple times (e.g., backtracking through a junction)
- Fixed conflict list showing incorrect count with empty list - conflict filtering was using wrong index type causing mismatched visibility checks
- Fixed junctions incorrectly having wait time - junctions now show "-" in the wait time column and never add wait time to journeys

# v0.1.7 - 2025-10-26

## Features
- Lines can now be sorted alphabetically, by creation order, or manually reordered by dragging
- View tabs can now be reordered by holding and dragging them to a new position
- Added keyboard shortcuts for quick tab switching - press 1-0 to switch between tabs (1 for Infrastructure, 2-0 for views)
- Added Distance Offset column type for CSV imports - allows importing cumulative distances that are automatically converted to inter-node distances
- View creation now supports multiple waypoints - create views that pass through any number of stations and junctions
- Stations and junctions can now be selected from a dropdown when creating views
- View creation now shows live preview of the path through waypoints, highlighted in blue on the infrastructure canvas

## Improvements
- Confirmation dialogs now support keyboard shortcuts - Enter to confirm, Escape to cancel
- Added disabled button styling to make it clearer when buttons cannot be clicked

## Bug Fixes
- Fixed last segment not rendering when importing CSV lines onto existing infrastructure
- Fixed conflict detection not detecting conflicts between late Sunday departures and early Monday departures
- Fixed CSV import not using auto-detected line name from filename for single-line imports
- Fixed error list items shifting right and overflowing when hovered
- Fixed false head-on conflicts on multi-track railways when trains travel in opposite directions on different tracks
- Fixed time and duration inputs accepting invalid values - invalid input is now reset to the last valid value
- Fixed "Add stop to start" on return route adding stops to the end instead
- Fixed route creation unable to find paths that require passing through a junction, and then re-entering that same junction from a different direction

# v0.1.6 - 2025-10-25

## Bug Fixes
- Fixed line editor closing when modifying line properties, as a result views will not auto-update on line changes. A better solution for this will come in a future update, until then you must recreate your views after adding stations.

# v0.1.5 - 2025-10-25

## Improvements
- Added proper support for lines that revisit the same station or track segment
- Improved visibility of grid in infrastructure editor
- Added double-click to edit line in line controls sidebar
- Added automatic view regeneration when source line or infrastructure changes
- Added duplicate line feature - creates a copy of a line with smart name incrementing
- Reorganized line controls with ellipsis menu - actions now in dropdown menu for cleaner interface

## Bug Fixes
- Fixed an issue where opening a view from a line would sometimes not work
- Fixed JTrainGraph import not parsing days of week from train data
- Fixed line list in sidebar resetting when toggling line visibility or making other changes
- Fixed time inputs using browser's native time picker which forced 12-hour format on some systems
- Fixed delete button not appearing on new last stop after deleting the previous last stop in line editor
- Fixed infrastructure view not showing CSV-imported stations in new projects
- Fixed long station names overlapping train graph area

# v0.1.4 - 2025-10-24

## Bug Fixes
- Fixed route creation assigning all segments to track 0 instead of selecting appropriate tracks based on route direction - existing projects will be automatically fixed on load
- Fixed station labels not appearing in infrastructure view when station has no connections
- Fixed new project creation including data from the current project

## Improvements
- CSV import now supports "Don't create new infrastructure" mode - uses pathfinding on existing tracks to create routes between CSV stations without creating new stations or tracks
- CSV import now supports infrastructure-only mode - import CSVs with just stations and infrastructure columns (platform, track, distance) without time data to create network infrastructure
- Added comprehensive CSV import documentation (see docs/csv-import-guide.md) covering all column types, import modes, pattern repeat feature, and examples
- CSV import dialog now includes a link to the documentation guide for easy reference
- Manual departures can now repeat at a specified interval until a given time or end of day - allows creating repeating services without switching to automatic scheduling
- Manual departures section is now always visible in line editor, even when automatic scheduling is enabled, making the feature more discoverable and allowing hybrid scheduling
- Added customizable keyboard shortcuts - shortcuts can now be rebound in Settings > Keyboard Shortcuts with conflict detection and platform-specific defaults (Cmd on Mac, Ctrl on Windows/Linux)
- Added 'Add Connection' section to station editor allowing you to easily connect stations without manually drawing tracks
- Windows now remember their last position and reopen at that position (position is unique per window type: station editor, track editor, etc.)
- Added Progressive Web App support - app can now be installed and works offline with full asset caching
- Reduced tab height and header padding for a more compact interface

# v0.1.3 - 2025-10-23

Improvements have been made to the autolayout however it still exhibits some problematic behaviour, I will continue working on it but I did not wish to hold up all the other changes in this release.  

## Bug Fixes
- Fixed view creation from lines ignoring disabled junction connections (pathfinding now respects junction routing rules)
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
- Added project settings dialog with track handedness configuration (right-hand or left-hand traffic) affecting default platform and track direction assignments during imports and route creation
- Infrastructure autolayout now produces clearer network layouts with less overlapping lines and better visual separation between different routes
- Infrastructure view now remembers your pan and zoom position when switching between tabs
- Dragged stations now snap to grid even when autolayout is disabled
- Added subtle grid pattern to infrastructure view showing 30px snap points
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
- Horizontal scaling in time graph now triggered by scrolling with cursor over time labels (replacing Alt+scroll which conflicted with browser shortcuts)
- CSV import now automatically fills in the filename (without extension) as the line name when importing a single line
- Project import now automatically uses the filename (without extension) as the project name
- Main Line view now automatically regenerates when infrastructure changes (new stations, junctions, or tracks added)
- Added zoom in/out keyboard shortcuts: = to zoom in, - to zoom out (numpad +/- also supported)

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

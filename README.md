# SL map tools

This is a small set of Rust libraries and a CLI to generate
Second Life maps using the map tile CDN and the APIs for
resolving region names to grid coordinates and vice versa.

It also includes code to parse USB notecards as used in the sailing
and flying communities by many HUDs in SL.

The APIs and map tiles are cached locally according to the cache headers
presented by the server.

You can use Preferences->Move & View->Map & Minimap->Show grid coordinates on
the worldmap in the Firestorm viewer (and probably others) to show grid
coordinates, Da Boom, the oldest region, is at (1000, 1000).

# Generate a map from a grid rectangle

To generate a map for a given rectangle of grid coordinates you need the
coordinates for the lower left corner (the lowest coordinates on each axis)
and for the upper right corner (the highest coordinates on each axis).

You also should specify the same cache directory on every call and give an
output filename as well as a limit on the image width and height in pixels.

The tool automatically adjusts the resolution down to exactly match the number
of regions in the grid rectangle at the most detailed zoom level that fits into
these maximum dimensions.


```
sl_map_cli --cache-dir cache from-grid-rectangle --lower-left-x 380 --lower-left-y 380 --upper-right-x 1500 --upper-right-y 1500 --max-width 2048 --max-height 2048 --output-file test_map.jpg
```

# Generate a map from a USB notecard

The USB notecard should be saved into a text file. A hex color code
should be specified for the route waypoints and lines (no arrows or
B-splines yet but that is one of my future plans).

All the notes on cache dir, output file and resolution apply equally to this
mode


```
sl_map_cli --cache-dir cache from-usb-notecard --usb-notecard usb_notecard.txt --color '#0f0' --max-width 2048 --max-height 2048 --output-file test_map.jpg
```

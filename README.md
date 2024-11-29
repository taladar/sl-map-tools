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

By default missing map tiles (as in map tiles where no regions exist in that
area) will appear as black while missing regions inside lower detail map tiles
are shown in a water-like color (#19485a and sometimes close ones due to
JPEG compression) by Linden Lab.

It is possible to change the fill color for missing map tiles with the
--missing-map-tile-color option.

It is also possible to fill the missing regions within map tiles though
determining which regions exist there has some performance impact.

To use this you can use the  --missing-region-color option.


```
sl_map_cli --cache-dir cache from-grid-rectangle --lower-left-x 380 --lower-left-y 380 --upper-right-x 1500 --upper-right-y 1500 --max-width 2048 --max-height 2048 --output-file test_map.jpg
```

If you want to use the generated map with a PPS HUD you can use the string
printed at the end as the description of the dot prim of your PPS HUD to avoid
having the manually (and less accurately) calibrate the HUD to fit
the map texture.

You can then use a long click on the dot to reset the scripts.

You can also use the aspect ratio printed to edit the PPS HUD to be the correct
size.

# Generate a map from a USB notecard

The USB notecard should be saved into a text file. A hex color code
should be specified for the route waypoints and lines (no arrows or
B-splines yet but that is one of my future plans).

All the notes on cache dir, output file and resolution apply equally to this
mode


```
sl_map_cli --cache-dir cache from-usb-notecard --usb-notecard usb_notecard.txt --color '#0f0' --max-width 2048 --max-height 2048 --output-file test_map.jpg
```

It is also possible to additionally save the map without the route to a separate file by specifying the --output-file-without-route option with another file name.

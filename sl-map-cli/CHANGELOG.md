## 0.1.4

add arrows before waypoints
remove waypoint squares
make spline between waypoints dotted

## 0.1.3

replace simple lines between waypoints with Catmull-Rom spline
fix bug that could lead to an error if a waypoint was too close to 0
(closer than the size of the rectangle drawn at waypoints)

## 0.1.2

allow optionally filling in missing regions with a color
allow optionally filling in missing map tiles with a color
allow saving of the USB notecard map without a route to an extra file
before the route is drawn on it

## 0.1.1

Print PPS HUD config string and size recommendations when generating a map

Fix max zoom level calculations, previously it used the minimum zoom level out
of the one calculated for x and y axes but lower zoom levels are higher detail
so it needed to be the maximum.

## 0.1.0

Initial Release

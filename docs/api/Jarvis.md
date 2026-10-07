# Jarvis

`net.runelite.api.model.Jarvis` — class render contract (§F1). Convex-hull clickboxes for picking.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/model/Jarvis.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/model/Jarvis.html

> Provides utility methods for computing the convex hull of a list of <em>n</em> points. <p> The implementation uses the Jarvis march algorithm and runs in O(nh) time in the worst case, where n is the number of points and h the number of points on the convex hull.

Declaration: `class Jarvis`

## Methods (5)

### `public static List<Point> convexHull(List<Point> points) {`
Computes and returns the convex hull of the passed points. <p> The size of the list must be at least 3, otherwise this method will return null.

### `public static SimplePolygon convexHull(int[] xs, int[] ys) {`
Computes and returns the convex hull of the passed points. <p> The size of the list must be at least 3, otherwise this method will return null.

### `private static int square(int x) {`

### `private static int findLeftMost(int[] xs, int[] ys, int length) {`

### `private static long crossProduct(int px, int py, int qx, int qy, int rx, int ry) {`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._

# SimplePolygon

`net.runelite.api.geometry.SimplePolygon` — class render contract (§F1). Hull/selection polygon container.

Source: `/Users/tylercovalt/Documents/runelite-master/runelite-api/src/main/java/net/runelite/api/geometry/SimplePolygon.java`
Apidocs: https://static.runelite.net/runelite-api/apidocs/net/runelite/api/geometry/SimplePolygon.html

> A simple list of vertices that can be append or prepended to

Declaration: `class SimplePolygon implements Shape`

## Methods (27)

### `public SimplePolygon() {`

### `public SimplePolygon(int[] x, int[] y, int length) {`

### `public void pushLeft(int xCoord, int yCoord) {`

### `public void popLeft() {`

### `protected void expandLeft(int grow) {`

### `public void pushRight(int xCoord, int yCoord) {`

### `public void popRight() {`

### `protected void expandRight(int grow) {`

### `public int getX(int index) {`

### `public int getY(int index) {`

### `public int size() {`

### `public List<Point> toRuneLitePointList() {`

### `public void copyTo(int[] xDest, int[] yDest, int offset) {`

### `public void appendTo(SimplePolygon other) {`

### `public void reverse() {`

### `public void intersectWithConvex(SimplePolygon convex) {`
Clips the polygon with the passed convex polygon

### `public Rectangle getBounds() {`

### `public Rectangle2D getBounds2D() {`

### `public boolean contains(double cx, double cy) {`

### `private int crossings(double cx, double cy, boolean swap) {`

### `public boolean contains(Point2D p) {`

### `public boolean intersects(double x0, double y0, double w, double h) {`

### `public boolean intersects(Rectangle2D r) {`

### `public boolean contains(double x, double y, double w, double h) {`

### `public boolean contains(Rectangle2D r) {`

### `public PathIterator getPathIterator(AffineTransform at) {`

### `public PathIterator getPathIterator(AffineTransform at, double flatness) {`

## Fields (5)

- `private static final int GROW = 16;`
- `protected int[] x, y;`
- `protected int[] x, y;`
- `protected int left, right;`
- `protected int left, right;`

_Generated from the local Oct-2026 snapshot by `tools/runelite-mcp/gen_api_docs.py`. Regenerate after updating the snapshot._

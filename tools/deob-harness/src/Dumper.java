import java.util.Arrays;

/**
 * Headless harness over the deobfuscated client (default package, so
 * package-private members are reachable). Dumps machine-checked golden data
 * for the Rust port. Each section prints `KEY=value` lines.
 *
 * Compile:
 *   javac -d /tmp/deobwork/classes -sourcepath RS/src/main/java:INJ/src/main/java \
 *     tools/deob-harness/src/Dumper.java
 * Run:
 *   java -cp /tmp/deobwork/classes Dumper
 */
public class Dumper {
	static String ia(int[] a) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < a.length; ++i) {
			if (i > 0) sb.append(',');
			sb.append(a[i]);
		}
		return sb.toString();
	}

	static String iaa(int[][] a) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < a.length; ++i) {
			if (i > 0) sb.append(';');
			sb.append(ia(a[i]));
		}
		return sb.toString();
	}

	public static void main(String[] args) {
		// T1: tile shape tables (Scene static init)
		System.out.println("tileShape2D=" + iaa(Scene.tileShape2D));
		System.out.println("tileRotation2D=" + iaa(Scene.tileRotation2D));

		// T2: wall orientation/offset tables
		System.out.println("field800=" + ia(Tiles.field800));
		System.out.println("field804=" + ia(Tiles.field804));
		System.out.println("field802=" + ia(Tiles.field802));
		System.out.println("field798=" + ia(Tiles.field798));
		System.out.println("field803=" + ia(Tiles.field803));
		System.out.println("field805=" + ia(Tiles.field805));

		// T3: SceneTileModel triangulation for shapes 0..12 x rotations 0..3.
		// ctor(shape, rot, tex, tileX, tileY, hSW,hSE,hNE,hNW,
		//        uSW,uSE,uNE,uNW, oSW,oSE,oNE,oNW, underRgb, overRgb)
		// Flat heights (all 64): underlay colors 1000..1003, overlay 2000..2003.
		for (int shape = 0; shape <= 12; ++shape) {
			for (int rot = 0; rot <= 3; ++rot) {
				SceneTileModel m = new SceneTileModel(shape, rot, -1,
					0, 0, 64, 64, 64, 64,
					1000, 1001, 1002, 1003,
					2000, 2001, 2002, 2003,
					1111, 2222);
				System.out.println("tri shape=" + shape + " rot=" + rot
					+ " nverts=" + m.vertexX.length
					+ " vx=" + ia(m.vertexX)
					+ " vy=" + ia(m.vertexY)
					+ " vz=" + ia(m.vertexZ)
					+ " faces=" + m.faceX.length
					+ " fx=" + ia(m.faceX)
					+ " fy=" + ia(m.faceY)
					+ " fz=" + ia(m.faceZ)
					+ " ca=" + ia(m.triangleColorA)
					+ " cb=" + ia(m.triangleColorB)
					+ " cc=" + ia(m.triangleColorC)
					+ " tex=" + (m.triangleTextureId == null ? "null" : ia(m.triangleTextureId))
					+ " flat=" + m.isFlat);
			}
		}

		// T4: sloped tile (SW=64 SE=96 NE=128 NW=80) shape 0 rot 0 — midpoint averaging check
		{
			SceneTileModel m = new SceneTileModel(0, 0, -1,
				2, 3, 64, 96, 128, 80,
				1000, 1001, 1002, 1003,
				2000, 2001, 2002, 2003,
				1111, 2222);
			System.out.println("slope vx=" + ia(m.vertexX) + " vy=" + ia(m.vertexY)
				+ " vz=" + ia(m.vertexZ) + " ca=" + ia(m.triangleColorA));
		}

		// T5: color functions across representative inputs
		int[] hs = {0, 255, 0xFF0000, 0x00FF00, 0x0000FF, 0x808080, 0xC8A048, 0x123456};
		for (int rgb : hs) {
			FloorUnderlayDefinition u = new FloorUnderlayDefinition();
			u.setHsl(rgb);
			System.out.println("sethsl rgb=" + rgb + " hue=" + u.hue + " sat=" + u.saturation
				+ " light=" + u.lightness + " mult=" + u.hueMultiplier);
		}
		int[][] m817 = {{0, 0, 0}, {255, 255, 255}, {128, 200, 100}, {10, 250, 220}, {200, 30, 250}};
		for (int[] t : m817) {
			System.out.println("m817 h=" + t[0] + " s=" + t[1] + " l=" + t[2]
				+ " -> " + class39.method817(t[0], t[1], t[2]));
		}
		int[][] m2086 = {{0x2345, 96}, {0xFFFF, 0}, {0xFFFF, 200}, {0x1000, 96}, {-1, 96}};
		for (int[] t : m2086) {
			System.out.println("m2086 hsl=" + t[0] + " b=" + t[1] + " -> " + class57.method2086(t[0], t[1]));
		}
		int[][] m5263 = {{0x2345, 100}, {0xFFFF, 200}, {0x1000, 0}, {0x1000, 300}};
		for (int[] t : m5263) {
			System.out.println("m5263 hsl=" + t[0] + " l=" + t[1] + " -> " + ModelData.method5263(t[0], t[1]));
		}
		for (int l : new int[]{0, 1, 2, 100, 126, 127, 200}) {
			System.out.println("m5264 l=" + l + " -> " + ModelData.method5264(l));
		}

		// T6: contourGround on synthetic heightmaps
		{
			int[][] flat = new int[8][8];
			for (int[] row : flat) Arrays.fill(row, 100);
			Model m = new Model();
			m.verticesCount = 1;
			m.verticesX = new int[]{0};
			m.verticesY = new int[]{0};
			m.verticesZ = new int[]{0};
			m.xzRadius = 0;
			m.height = 100;
			Model r = m.contourGround(flat, 256, 100, 256, true, 0);
			System.out.println("contour flat y=" + r.verticesY[0]);
			int[][] slope = new int[8][8];
			for (int x = 0; x < 8; ++x)
				for (int z = 0; z < 8; ++z)
					slope[x][z] = x * 10 + z;
			Model m2 = new Model();
			m2.verticesCount = 1;
			m2.verticesX = new int[]{0};
			m2.verticesY = new int[]{0};
			m2.verticesZ = new int[]{0};
			m2.xzRadius = 0;
			m2.height = 100;
			Model r2 = m2.contourGround(slope, 256, 50, 256, true, 0);
			System.out.println("contour slope y=" + r2.verticesY[0]);
		}

		// T7: toModel lighting on a synthetic single-triangle model (loc rig)
		{
			ModelData md = new ModelData();
			md.verticesCount = 3;
			md.verticesX = new int[]{0, 128, 0};
			md.verticesY = new int[]{0, 0, 0};
			md.verticesZ = new int[]{0, 0, 128};
			md.faceCount = 1;
			md.indices1 = new int[]{0};
			md.indices2 = new int[]{1};
			md.indices3 = new int[]{2};
			md.faceColors = new short[]{0x1234};
			Model lit = md.toModel(64, 768, -50, -10, -50);
			System.out.println("tolit c1=" + lit.faceColors1[0]
				+ " c2=" + lit.faceColors2[0] + " c3=" + lit.faceColors3[0]);
		}
		// T8: WallDecoration.method6262 nudge table — all orientation flags seen
		// in the wild (1,2,4,8 straight; 16,32,64,128 diagonal; 256 two-sided)
		// x offsets (16,0), (0,-16), (8,-8). yOffset/field3194 keep raw inputs;
		// xOffset/field3196 get the +/-1 nudge.
		{
			int[] orients = {1, 2, 4, 8, 16, 32, 64, 128, 256};
			int[][] offs = {{16, 0}, {0, -16}, {8, -8}};
			for (int o : orients) {
				for (int[] d : offs) {
					WallDecoration w = new WallDecoration();
					w.orientation = o;
					w.method6262(d[0], d[1]);
					System.out.println("nudge o=" + o + " in=" + d[0] + "," + d[1]
						+ " xOffset=" + w.xOffset + " f3196=" + w.field3196
						+ " yOffset=" + w.yOffset + " f3194=" + w.field3194);
				}
			}
		}

		// T9: live Scene storage — exact class150 call shapes for wall types
		// 0,1,2,3 + decor types 4-8 + floor 22 on tile (10,20) plane 1.
		{
			Scene sc = new Scene(0, 4, 104, 104, 0, TileRenderMode.field2669,
				new int[4][105][105]);
			Renderable r1 = new Renderable() {};
			Renderable r2 = new Renderable() {};
			long tag = 0xABCDL;
			for (int o = 0; o <= 3; ++o) {
				sc.newBoundaryObject(1, 10, 20, 100, r1, null, Tiles.field800[o], 0, tag, (o << 6) + 0);
				BoundaryObject b = sc.tiles[1][10][20].boundaryObject;
				System.out.println("wall t=0 o=" + o + " x=" + b.x + " y=" + b.y + " z=" + b.z
					+ " oA=" + b.orientationA + " oB=" + b.orientationB);
				sc.newBoundaryObject(1, 10, 20, 100, r1, null, Tiles.field804[o], 0, tag, (o << 6) + 1);
				b = sc.tiles[1][10][20].boundaryObject;
				System.out.println("wall t=1 o=" + o + " x=" + b.x + " y=" + b.y + " z=" + b.z
					+ " oA=" + b.orientationA + " oB=" + b.orientationB);
				int o2 = o + 1 & 3;
				sc.newBoundaryObject(1, 10, 20, 100, r1, r2, Tiles.field800[o], Tiles.field800[o2], tag, (o << 6) + 2);
				b = sc.tiles[1][10][20].boundaryObject;
				System.out.println("wall t=2 o=" + o + " x=" + b.x + " y=" + b.y + " z=" + b.z
					+ " oA=" + b.orientationA + " oB=" + b.orientationB);
				sc.newBoundaryObject(1, 10, 20, 100, r1, null, Tiles.field804[o], 0, tag, (o << 6) + 3);
				b = sc.tiles[1][10][20].boundaryObject;
				System.out.println("wall t=3 o=" + o + " x=" + b.x + " y=" + b.y + " z=" + b.z
					+ " oA=" + b.orientationA + " oB=" + b.orientationB);
				sc.newWallDecoration(1, 10, 20, 100, r1, null, Tiles.field800[o], 0, 0, 0, tag, (o << 6) + 4);
				WallDecoration w = sc.tiles[1][10][20].wallDecoration;
				System.out.println("decor t=4 o=" + o + " x=" + w.x + " y=" + w.y + " z=" + w.z
					+ " o=" + w.orientation + " o2=" + w.orientation2
					+ " xOff=" + w.xOffset + " f3196=" + w.field3196);
				sc.newWallDecoration(1, 10, 20, 100, r1, null, Tiles.field800[o], 0,
					Tiles.field802[o] * 16, Tiles.field798[o] * 16, tag, (o << 6) + 5);
				w = sc.tiles[1][10][20].wallDecoration;
				System.out.println("decor t=5 o=" + o + " x=" + w.x + " y=" + w.y + " z=" + w.z
					+ " o=" + w.orientation + " o2=" + w.orientation2
					+ " xOff=" + w.xOffset + " f3196=" + w.field3196);
				sc.newWallDecoration(1, 10, 20, 100, r1, null, 256, o,
					Tiles.field803[o] * 8, Tiles.field805[o] * 8, tag, (o << 6) + 6);
				w = sc.tiles[1][10][20].wallDecoration;
				System.out.println("decor t=6 o=" + o + " x=" + w.x + " y=" + w.y + " z=" + w.z
					+ " o=" + w.orientation + " o2=" + w.orientation2
					+ " xOff=" + w.xOffset + " f3196=" + w.field3196);
				int ob = o + 2 & 3;
				sc.newWallDecoration(1, 10, 20, 100, r1, null, 256, ob, 0, 0, tag, (o << 6) + 7);
				w = sc.tiles[1][10][20].wallDecoration;
				System.out.println("decor t=7 o=" + o + " x=" + w.x + " y=" + w.y + " z=" + w.z
					+ " o=" + w.orientation + " o2=" + w.orientation2
					+ " xOff=" + w.xOffset + " f3196=" + w.field3196);
				sc.newWallDecoration(1, 10, 20, 100, r1, r2, 256, o,
					Tiles.field803[o] * 8, Tiles.field805[o] * 8, tag, (o << 6) + 8);
				w = sc.tiles[1][10][20].wallDecoration;
				System.out.println("decor t=8 o=" + o + " x=" + w.x + " y=" + w.y + " z=" + w.z
					+ " o=" + w.orientation + " o2=" + w.orientation2
					+ " xOff=" + w.xOffset + " f3196=" + w.field3196);
			}
			sc.newFloorDecoration(1, 10, 20, 100, r1, tag, 22);
			FloorDecoration f = sc.tiles[1][10][20].floorDecoration;
			System.out.println("floor t=22 x=" + f.x + " y=" + f.y + " z=" + f.z);
		}

		// T10: game-object path storage — roof-like 1x1 type 14, type-9-like
		// diagonal 1x1, and a 2x1 footprint spanning tiles (edge masks).
		{
			Scene sc = new Scene(0, 4, 104, 104, 0, TileRenderMode.field2669,
				new int[4][105][105]);
			Renderable r1 = new Renderable() {};
			long tag = 0xABCDL;
			sc.method5564(1, 10, 20, 100, 1, 1, r1, 0, tag, (2 << 6) + 14);
			GameObject g = sc.tiles[1][10][20].gameObjects[0];
			System.out.println("roof t=14 cx=" + g.centerX + " cy=" + g.centerY
				+ " z=" + g.z + " sx=" + g.startX + " ex=" + g.endX
				+ " edge=" + sc.tiles[1][10][20].gameObjectEdgeMasks[0]
				+ " edgeOr=" + sc.tiles[1][10][20].gameObjectsEdgeMask);
			sc.method5564(1, 30, 40, 100, 1, 1, r1, 0, tag, (1 << 6) + 9);
			g = sc.tiles[1][30][40].gameObjects[0];
			System.out.println("walldiag t=9 cx=" + g.centerX + " cy=" + g.centerY
				+ " z=" + g.z + " edge=" + sc.tiles[1][30][40].gameObjectEdgeMasks[0]);
			sc.method5564(1, 50, 60, 100, 2, 1, r1, 0, tag, (0 << 6) + 10);
			GameObject g0 = sc.tiles[1][50][60].gameObjects[0];
			GameObject g1 = sc.tiles[1][51][60].gameObjects[0];
			System.out.println("gate2x1 same=" + (g0 == g1)
				+ " cx=" + g0.centerX + " cy=" + g0.centerY
				+ " e0=" + sc.tiles[1][50][60].gameObjectEdgeMasks[0]
				+ " e1=" + sc.tiles[1][51][60].gameObjectEdgeMasks[0]);
			// capacity: 5 per tile
			boolean sixth = true;
			for (int k = 0; k < 6; ++k) {
				sixth = sc.method5564(1, 70, 70, 100, 1, 1, r1, 0, tag, k);
			}
			System.out.println("cap5 sixthAccepted=" + sixth
				+ " count=" + sc.tiles[1][70][70].gameObjectsCount);
		}
		System.out.println("DONE");
	}
}

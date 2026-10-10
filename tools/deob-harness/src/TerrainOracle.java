import java.io.BufferedReader;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;

/**
 * Executable oracle for TERRAIN-004 and the terrain loader.
 *
 * Runs the pinned client's real {@code class264.loadTerrain}, {@code ScriptFrame.method749}
 * (empty-region fill) and {@code class470.method9712} (terrain builder) against caller-supplied
 * floor definitions and raw terrain streams, then dumps heights, per-tile paint/model colors,
 * minimum planes, and link-below state.
 *
 * Input format (one record per line, `#` comments allowed):
 *   brightness <double>              palette brightness (default 0.8)
 *   underlay <id> <hex>              raw FloorUnderlayDefinition bytes
 *   overlay <id> <hex>               raw FloorOverlayDefinition bytes
 *   texavg <id> <rgb>                texture average RGB returned by the stub texture loader
 *   noise <x> <y>                    noise offsets passed as loadTerrain's (var5,var6) base
 *   land <sceneX> <sceneY> <hex>     decoded terrain stream for one 64x64 region
 *   empty <sceneX> <sceneY>          region without land data (ScriptFrame.method749 fill)
 *   shadow <plane> <x> <y> <value>   direct Tiles_underlays2 write (stand-in for placed locs)
 *   rotation <0..3>                  loadTerrain rotation argument (default 0)
 *
 * Compile (see run-terrain-oracle.sh):
 *   javac -sourcepath RS:INJ tools/deob-harness/src/TerrainOracle.java
 */
public class TerrainOracle {
	static byte[] hex(String s) {
		byte[] out = new byte[s.length() / 2];
		for (int i = 0; i < out.length; ++i) {
			out[i] = (byte) Integer.parseInt(s.substring(i * 2, i * 2 + 2), 16);
		}
		return out;
	}

	static String ia(int[] a) {
		if (a == null) {
			return "-";
		}
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < a.length; ++i) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(a[i]);
		}
		return sb.toString();
	}

	public static void main(String[] args) throws IOException {
		if (args.length != 1) {
			System.err.println("usage: TerrainOracle <input-file>");
			System.exit(2);
		}

		double brightness = 0.8D;
		int rotation = 0;
		int noiseX = 0;
		int noiseY = 0;
		Map<Integer, Integer> texAvg = new HashMap<>();
		java.util.List<String[]> records = new java.util.ArrayList<>();
		try (BufferedReader reader = Files.newBufferedReader(Paths.get(args[0]), StandardCharsets.UTF_8)) {
			String line;
			while ((line = reader.readLine()) != null) {
				line = line.trim();
				if (line.isEmpty() || line.startsWith("#")) {
					continue;
				}
				records.add(line.split("\\s+"));
			}
		}

		for (String[] r : records) {
			switch (r[0]) {
				case "brightness":
					brightness = Double.parseDouble(r[1]);
					break;
				case "rotation":
					rotation = Integer.parseInt(r[1]);
					break;
				case "noise":
					noiseX = Integer.parseInt(r[1]);
					noiseY = Integer.parseInt(r[2]);
					break;
				case "texavg":
					texAvg.put(Integer.parseInt(r[1]), Integer.parseInt(r[2]));
					break;
				default:
					break;
			}
		}

		Rasterizer3D.buildPalette(brightness);
		final Map<Integer, Integer> avg = texAvg;
		Rasterizer3D.setTextureLoader(new TextureLoader() {
			public int[] getTexturePixels(int id) {
				return null;
			}

			public int getAverageTextureRGB(int id) {
				Integer v = avg.get(id);
				if (v == null) {
					throw new IllegalStateException("missing texavg for texture " + id);
				}
				return v;
			}

			public boolean isLowDetail(int id) {
				return false;
			}
		});

		for (String[] r : records) {
			if (r[0].equals("underlay")) {
				int id = Integer.parseInt(r[1]);
				FloorUnderlayDefinition def = new FloorUnderlayDefinition();
				def.decode(new Buffer(hex(r[2])), id);
				def.postDecode();
				FloorUnderlayDefinition.FloorUnderlayDefinition_cached.put(def, (long) id);
			} else if (r[0].equals("overlay")) {
				int id = Integer.parseInt(r[1]);
				FloorOverlayDefinition def = new FloorOverlayDefinition();
				def.decode(new Buffer(hex(r[2])), id);
				def.postDecode();
				FloorOverlayDefinition.FloorOverlayDefinition_cached.put(def, (long) id);
			}
		}

		SoundSystem.method3227();
		WorldView wv = new WorldView(0, 104, 104, 0, TileRenderMode.field2669);

		for (String[] r : records) {
			if (r[0].equals("land")) {
				int sx = Integer.parseInt(r[1]);
				int sy = Integer.parseInt(r[2]);
				loadRegion(wv, hex(r[3]), sx, sy, noiseX, noiseY, rotation);
			}
		}
		for (String[] r : records) {
			if (r[0].equals("empty")) {
				int sx = Integer.parseInt(r[1]);
				int sy = Integer.parseInt(r[2]);
				ScriptFrame.method749(wv, sx, sy, 64, 64);
			}
		}
		for (String[] r : records) {
			if (r[0].equals("shadow")) {
				Tiles.Tiles_underlays2[Integer.parseInt(r[1])][Integer.parseInt(r[2])][Integer.parseInt(r[3])] =
					(byte) Integer.parseInt(r[4]);
			}
		}

		class470.method9712(wv);

		PrintStream out = new PrintStream(System.out, false, "UTF-8");
		out.println("rnd hue=" + Tiles.rndHue + " lightness=" + Tiles.rndLightness);
		out.println("tiles_min_plane=" + Tiles.Tiles_minPlane);
		for (int p = 0; p < 4; ++p) {
			for (int x = 0; x < 105; ++x) {
				out.println("h " + p + " " + x + " " + ia(wv.tileHeights[p][x]));
			}
		}
		for (int p = 0; p < 4; ++p) {
			for (int x = 0; x < 104; ++x) {
				StringBuilder row = new StringBuilder();
				for (int y = 0; y < 104; ++y) {
					row.append(wv.tileSettings[p][x][y]).append(y + 1 < 104 ? "," : "");
				}
				out.println("s " + p + " " + x + " " + row);
			}
		}
		for (int p = 0; p < 4; ++p) {
			for (int x = 0; x < 104; ++x) {
				for (int y = 0; y < 104; ++y) {
					Tile t = wv.scene.tiles[p][x][y];
					if (t != null) {
						dumpTile(out, "tile", p, x, y, t);
						if (t.linkedBelowTile != null) {
							dumpTile(out, "below", p, x, y, t.linkedBelowTile);
						}
					}
				}
			}
		}
		out.flush();
	}

	/** Mirrors class337.method7281 without the collision-map reset. */
	static void loadRegion(WorldView wv, byte[] bytes, int sceneX, int sceneY, int noiseX, int noiseY, int rotation) {
		Buffer buf = new Buffer(bytes);
		for (int plane = 0; plane < 4; ++plane) {
			for (int x = 0; x < 64; ++x) {
				for (int y = 0; y < 64; ++y) {
					int tx = x + sceneX;
					int ty = sceneY + y;
					class264.loadTerrain(wv, buf, plane, tx, ty, tx + noiseX, noiseY + ty, rotation);
				}
			}
		}
		int flags = buf.offset * 2108391709 < buf.array.length ? buf.readUnsignedByte() : 0;
		if ((flags & 1) != 0) {
			for (int i = 0; i < 64; ++i) {
				for (int j = 0; j < 64; ++j) {
					class148.method3945(buf);
				}
			}
		}
		System.err.println("region " + sceneX + "," + sceneY + " consumed offset=" + buf.offset * 2108391709
			+ " of " + buf.array.length + " flags=" + flags);
	}

	static void dumpTile(PrintStream out, String tag, int p, int x, int y, Tile t) {
		StringBuilder sb = new StringBuilder();
		sb.append(tag).append(' ').append(p).append(' ').append(x).append(' ').append(y);
		sb.append(" plane=").append(t.plane).append(" orig=").append(t.originalPlane);
		sb.append(" min=").append(t.minPlane);
		sb.append(" linked=").append(t.linkedBelowTile != null ? 1 : 0);
		SceneTilePaint paint = t.paint;
		if (paint != null) {
			sb.append(" paint=").append(paint.swColor).append(',').append(paint.seColor).append(',')
				.append(paint.neColor).append(',').append(paint.nwColor)
				.append(" tex=").append(paint.texture).append(" rgb=").append(paint.rgb)
				.append(" flat=").append(paint.isFlat ? 1 : 0);
		}
		SceneTileModel m = t.model;
		if (m != null) {
			sb.append(" model shape=").append(m.shape).append(" rot=").append(m.rotation)
				.append(" flat=").append(m.isFlat ? 1 : 0)
				.append(" under=").append(m.underlayRgb).append(" over=").append(m.overlayRgb)
				.append(" vx=").append(ia(m.vertexX)).append(" vy=").append(ia(m.vertexY))
				.append(" vz=").append(ia(m.vertexZ))
				.append(" ca=").append(ia(m.triangleColorA)).append(" cb=").append(ia(m.triangleColorB))
				.append(" cc=").append(ia(m.triangleColorC))
				.append(" fx=").append(ia(m.faceX)).append(" fy=").append(ia(m.faceY))
				.append(" fz=").append(ia(m.faceZ))
				.append(" ft=").append(ia(m.triangleTextureId));
		}
		out.println(sb);
	}
}

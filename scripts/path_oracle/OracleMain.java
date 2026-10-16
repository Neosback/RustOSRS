import com.rspsi.tools.PathOverlayFitter;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * Prints the EikenB00m editor's path fit for a fixed set of drawn paths, so the Rust port can be
 * compared with the original's output exactly. See scripts/path_oracle/run.sh.
 */
public class OracleMain {
	record Case(String name, double[][] points, double width, double edge, boolean smooth) {}

	static double[][] pts(double... xy) {
		double[][] out = new double[xy.length / 2][];
		for (int i = 0; i < out.length; i++)
			out[i] = new double[] { xy[2 * i] + 0.5, xy[2 * i + 1] + 0.5 };
		return out;
	}

	public static void main(String[] args) {
		int ox = Integer.parseInt(args[0]);
		int oy = Integer.parseInt(args[1]);
		List<Case> cases = new ArrayList<>();
		cases.add(new Case("single-point", pts(10, 10), 3, 1.5, true));
		cases.add(new Case("single-wide", pts(10, 10), 7, 1.5, true));
		cases.add(new Case("horizontal-3", pts(4, 10, 20, 10), 3, 1.5, true));
		cases.add(new Case("horizontal-1", pts(4, 10, 20, 10), 1, 1.5, true));
		cases.add(new Case("horizontal-2.5", pts(4, 10, 20, 10), 2.5, 1.5, true));
		cases.add(new Case("vertical-5", pts(10, 4, 10, 22), 5, 1.5, true));
		cases.add(new Case("diagonal-3", pts(4, 4, 22, 16), 3, 1.5, true));
		cases.add(new Case("diagonal-5-noedge", pts(4, 4, 22, 16), 5, 0, true));
		cases.add(new Case("diagonal-5-edge4", pts(4, 4, 22, 16), 5, 4, true));
		cases.add(new Case("shallow-4", pts(4, 4, 30, 9), 4, 1.5, true));
		cases.add(new Case("curve-3pts", pts(4, 4, 14, 14, 24, 4), 3, 1.5, true));
		cases.add(new Case("curve-5pts-wide", pts(4, 4, 10, 16, 16, 6, 22, 18, 30, 8), 6, 1.5, true));
		cases.add(new Case("straight-lines-L", pts(4, 4, 20, 4, 20, 18), 3, 1.5, false));
		cases.add(new Case("zigzag-straight", pts(4, 4, 12, 12, 20, 4, 28, 12), 4, 1, false));
		cases.add(new Case("loop-back", pts(10, 10, 20, 10, 20, 16, 10, 16, 10, 11), 3, 1.5, true));
		cases.add(new Case("duplicate-points", pts(8, 8, 8, 8, 18, 12), 3, 1.5, true));
		cases.add(new Case("width-20", pts(8, 8, 28, 20), 20, 1.5, true));
		cases.add(new Case("half-tile-width", pts(4, 7, 24, 9), 0.5, 1.5, true));
		for (Case c : cases) {
			List<double[]> points = new ArrayList<>();
			for (double[] p : c.points())
				points.add(new double[] { p[0] + ox, p[1] + oy });
			List<double[]> line = c.smooth() ? PathOverlayFitter.smooth(points) : points;
			Map<Integer, PathOverlayFitter.Fit> fits = PathOverlayFitter.fit(line, c.width(), 1 << 15, 1 << 15, c.edge());
			TreeMap<Long, PathOverlayFitter.Fit> sorted = new TreeMap<>();
			fits.forEach((key, fit) -> sorted.put(((long) PathOverlayFitter.tileX(key) << 32) | PathOverlayFitter.tileY(key), fit));
			System.out.println("case " + c.name() + " width=" + c.width() + " edge=" + c.edge() + " smooth=" + c.smooth());
			for (double[] p : points)
				System.out.println("point " + p[0] + " " + p[1]);
			sorted.forEach((key, fit) ->
					System.out.println("tile " + (key >> 32) + " " + (key & 0xffffffffL) + " " + fit.shape + " " + fit.rotation));
			System.out.println("end");
		}
	}
}

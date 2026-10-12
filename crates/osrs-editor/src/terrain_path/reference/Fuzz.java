import com.rspsi.tools.path.*;
import java.util.*;

public class Fuzz {
    static class Blocks implements CostGrid {
        final int x0, y0, x1, y1; final boolean all;
        Blocks(int x0, int y0, int x1, int y1, boolean all) { this.x0 = x0; this.y0 = y0; this.x1 = x1; this.y1 = y1; this.all = all; }
        @Override public CostGrid.Occupancy occupancy(int x, int y, int plane) {
            boolean in = x >= x0 && x <= x1 && y >= y0 && y <= y1;
            if (!in) return CostGrid.Occupancy.CLEAR;
            return all ? CostGrid.Occupancy.DECORATION : CostGrid.Occupancy.SOLID;
        }
        @Override public double slope(int x, int y, int plane) { return (x + y) % 17 == 0 ? 0.5 : 0.0; }
    }

    public static void main(String[] args) {
        Random r = new Random(2024);
        PathPlanner planner = new PathPlanner();
        TileBounds bounds = new TileBounds(0, 0, 160, 160);
        PathSettings.ObstaclePolicy[] pol = PathSettings.ObstaclePolicy.values();
        for (int c = 0; c < 40; c++) {
            int overlay = 1 + r.nextInt(20);
            double bw = 1 + r.nextDouble() * 7, ef = r.nextDouble() * 0.6, cw = r.nextDouble() * 0.8, ws = 2 + r.nextDouble() * 10;
            double wv = r.nextDouble() * 0.6, wns = 2 + r.nextDouble() * 10, en = r.nextBoolean() ? 0 : r.nextDouble() * 0.8, ens = 1 + r.nextDouble() * 5;
            int oct = 1 + r.nextInt(5); double oc = r.nextDouble() * 1.0; boolean reroute = r.nextInt(4) != 0; boolean ac = r.nextInt(4) != 0;
            PathProfile prof = PathProfile.builder("p", "P", overlay).baseWidth(bw).edgeFalloff(ef).centerlineWander(cw).wanderScale(ws).widthVariation(wv)
                .widthNoiseScale(wns).edgeNoise(en).edgeNoiseScale(ens).noiseOctaves(oct).obstacleClearance(oc).reroute(reroute).autoConnect(ac)
                .materialVariants(r.nextBoolean() ? List.of() : List.of(new PathProfile.MaterialVariant(overlay, 1.0 + r.nextInt(3)), new PathProfile.MaterialVariant(overlay + 1, 1.0 + r.nextInt(3)))).build();
            long seed = r.nextInt(100000);
            double sm = r.nextDouble(), ns = r.nextDouble() * 2, nsc = 0.5 + r.nextDouble() * 1.5, cl = r.nextBoolean() ? 0 : r.nextDouble() * 2;
            int ss = 4 + r.nextInt(7); PathSettings.ObstaclePolicy p = pol[r.nextInt(pol.length)];
            PathSettings st = PathSettings.builder().seed(seed).smoothness(sm).noiseStrength(ns).noiseScale(nsc).clearance(cl).supersample(ss).obstaclePolicy(p)
                .maximumSlope(r.nextInt(3) == 0 ? 0.3 : Double.MAX_VALUE).slopePenalty(r.nextDouble() * 4).routeCorridor(4 + r.nextInt(20)).build();
            int n = 2 + r.nextInt(6);
            double[] v = new double[n * 2];
            for (int i = 0; i < v.length; i += 2) { v[i] = 20 + r.nextDouble() * 110; v[i + 1] = 20 + r.nextDouble() * 110; }
            boolean block = r.nextBoolean();
            int bx = 30 + r.nextInt(80), by = 30 + r.nextInt(80), bw2 = r.nextInt(8), bh = r.nextInt(8);
            boolean all = r.nextBoolean();
            PathEnvironment env = block ? new PathEnvironment(bounds, new Blocks(bx, by, bx + bw2, by + bh, all), List.of()) : PathEnvironment.empty(bounds);
            // existing path pieces for snapping / preserve
            List<TilePaint> existing = new ArrayList<>();
            boolean ex = r.nextInt(3) == 0;
            if (ex) {
                int ex0 = 30 + r.nextInt(60), ey = 30 + r.nextInt(80);
                for (int x = ex0; x < ex0 + 15; x++) existing.add(new TilePaint(x, ey, 0, overlay, r.nextInt(12), r.nextInt(4), 1.0, "e"));
                env = new PathEnvironment(bounds, env.costGrid(), existing);
            }
            List<PathPoint> pl = new ArrayList<>();
            for (int i = 0; i < v.length; i += 2) pl.add(new PathPoint(v[i], v[i + 1]));
            PathNode a = new PathNode(new UUID(0, 1), pl.get(0).x(), pl.get(0).y(), 0);
            PathNode b = new PathNode(new UUID(0, 2), pl.get(n - 1).x(), pl.get(n - 1).y(), 0);
            long edgeId = 100 + c;
            PathEdge e = new PathEdge(new UUID(0, edgeId), a.id(), b.id(), n == 2 ? List.of() : List.copyOf(pl.subList(1, n - 1)), "p");
            PathPlan plan = planner.plan(new PathNetwork(new UUID(0, 9), "F", seed, List.of(a, b), List.of(e), List.of(prof)), st, env);
            // parameters, then results
            System.out.printf(Locale.ROOT, "CASE %d%n", c);
            System.out.printf(Locale.ROOT, "P %d %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %d %.17g %b %b%n", overlay, bw, ef, cw, ws, wv, wns, en, ens, oct, oc, reroute, ac);
            StringBuilder mv = new StringBuilder();
            for (PathProfile.MaterialVariant mvv : prof.materialVariants()) mv.append(mvv.overlayId()).append(',').append(mvv.weight()).append(' ');
            System.out.println("M " + mv);
            System.out.printf(Locale.ROOT, "S %d %.17g %.17g %.17g %.17g %d %s %s %.17g %.17g%n", seed, sm, ns, nsc, cl, ss, p, st.maximumSlope() == Double.MAX_VALUE ? "inf" : "0.3", st.slopePenalty(), st.routeCorridor());
            StringBuilder pts = new StringBuilder();
            for (PathPoint q : pl) pts.append(String.format(Locale.ROOT, "%.17g,%.17g ", q.x(), q.y()));
            System.out.println("L " + pts);
            System.out.println("E " + block + " " + bx + " " + by + " " + bw2 + " " + bh + " " + all);
            StringBuilder exs = new StringBuilder();
            for (TilePaint t : existing) exs.append(t.tileX()).append(',').append(t.tileY()).append(',').append(t.shape()).append(',').append(t.orientation()).append(' ');
            System.out.println("X " + exs);
            long h = 0xcbf29ce484222325L;
            for (TilePaint t : plan.paints()) {
                String line = String.format(Locale.ROOT, "%d %d %d %d %d %d %.6f %s\n", t.tileX(), t.tileY(), t.plane(), t.overlayId(), t.shape(), t.orientation(), t.coverage(), t.profileId());
                for (byte bb : line.getBytes(java.nio.charset.StandardCharsets.UTF_8)) { h ^= (bb & 0xffL); h *= 0x100000001b3L; }
            }
            System.out.println("N " + plan.paints().size() + " " + Long.toUnsignedString(h));
            for (PathWarning w : plan.warnings()) System.out.println("W " + w.severity() + " " + w.code() + " " + w.tileX() + " " + w.tileY() + " " + w.plane());
            System.out.println("END");
        }
    }
}

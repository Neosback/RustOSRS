import com.rspsi.tools.path.*;
import java.util.*;

public class Golden {
    static void dump(String name, PathPlan plan) {
        System.out.println("CASE " + name);
        for (TilePaint p : plan.paints()) {
            System.out.printf(Locale.ROOT, "T %d %d %d %d %d %d %.6f %s%n", p.tileX(), p.tileY(), p.plane(), p.overlayId(), p.shape(), p.orientation(), p.coverage(), p.profileId());
        }
        for (PathWarning w : plan.warnings()) {
            System.out.println("W " + w.severity() + " " + w.code() + " " + w.tileX() + " " + w.tileY() + " " + w.plane());
        }
        System.out.println("END");
    }

    static List<PathPoint> pts(double... v) {
        List<PathPoint> l = new ArrayList<>();
        for (int i = 0; i < v.length; i += 2) l.add(new PathPoint(v[i], v[i + 1]));
        return l;
    }

    static PathPlan stroke(PathPlanner planner, List<PathPoint> p, int plane, PathProfile prof, PathSettings st, PathEnvironment env, long edgeId) {
        PathNode a = new PathNode(new UUID(0, 1000 + edgeId), p.get(0).x(), p.get(0).y(), plane);
        PathNode b = new PathNode(new UUID(0, 2000 + edgeId), p.get(p.size() - 1).x(), p.get(p.size() - 1).y(), plane);
        List<PathPoint> mid = p.size() == 2 ? List.of() : List.copyOf(p.subList(1, p.size() - 1));
        PathEdge e = new PathEdge(new UUID(0, edgeId), a.id(), b.id(), mid, prof.id());
        return planner.plan(new PathNetwork(new UUID(0, 3000 + edgeId), "Stroke", st.seed(), List.of(a, b), List.of(e), List.of(prof)), st, env);
    }

    static class Blocks implements CostGrid {
        final int x0, y0, x1, y1;
        Blocks(int x0, int y0, int x1, int y1) { this.x0 = x0; this.y0 = y0; this.x1 = x1; this.y1 = y1; }
        @Override public CostGrid.Occupancy occupancy(int x, int y, int plane) {
            return x >= x0 && x <= x1 && y >= y0 && y <= y1 ? CostGrid.Occupancy.SOLID : CostGrid.Occupancy.CLEAR;
        }
    }

    public static void main(String[] args) {
        PathPlanner planner = new PathPlanner();
        TileBounds bounds = new TileBounds(0, 0, 200, 200);
        PathEnvironment open = PathEnvironment.empty(bounds);
        PathProfile prof = PathProfile.builder("default", "Default path", 5).build();
        PathSettings def = PathSettings.builder().build();
        PathSettings calm = PathSettings.builder().noiseStrength(0.0).build();
        PathSettings seeded = PathSettings.builder().seed(12345L).build();

        dump("straight_calm", stroke(planner, pts(20.5, 20.5, 60.5, 20.5), 0, prof, calm, open, 1));
        dump("diagonal_calm", stroke(planner, pts(20.5, 20.5, 50.5, 44.5), 0, prof, calm, open, 2));
        dump("curve_calm", stroke(planner, pts(20, 20, 40, 25, 55, 45, 70, 48, 90, 30), 0, prof, calm, open, 3));
        dump("curve_default", stroke(planner, pts(20, 20, 40, 25, 55, 45, 70, 48, 90, 30), 0, prof, def, open, 4));
        dump("curve_seeded", stroke(planner, pts(20, 20, 40, 25, 55, 45, 70, 48, 90, 30), 0, prof, seeded, open, 5));

        PathProfile wide = PathProfile.builder("wide", "Wide", 7).baseWidth(5.0).edgeNoise(0.4).widthVariation(0.3).build();
        dump("wide_noisy", stroke(planner, pts(30, 30, 50, 60, 80, 70), 0, wide, seeded, open, 6));

        PathProfile mixed = PathProfile.builder("mixed", "Mixed", 3)
            .materialVariants(List.of(new PathProfile.MaterialVariant(3, 2.0), new PathProfile.MaterialVariant(4, 1.0), new PathProfile.MaterialVariant(9, 1.0)))
            .materialPatchLength(5.0).build();
        dump("mixed_materials", stroke(planner, pts(20, 100, 120, 100), 0, mixed, seeded, open, 7));

        PathEnvironment walled = new PathEnvironment(bounds, new Blocks(50, 15, 52, 30), List.of());
        dump("reroute", stroke(planner, pts(20.5, 22.5, 80.5, 22.5), 0, prof, def, walled, 8));

        List<TilePaint> existing = new ArrayList<>();
        for (int x = 40; x < 60; x++) existing.add(new TilePaint(x, 80, 0, 5, 0, 0, 1.0, "existing"));
        PathEnvironment joined = new PathEnvironment(bounds, CostGrid.EMPTY, existing);
        dump("autoconnect", stroke(planner, pts(20.5, 70.5, 38.5, 78.5), 0, prof, def, joined, 9));
        dump("preserve", stroke(planner, pts(30.5, 80.5, 70.5, 80.5), 0, prof, calm, joined, 10));

        // A junction: three edges meeting at one node.
        UUID nid = new UUID(1, 1), a = new UUID(1, 2), b = new UUID(1, 3), c = new UUID(1, 4);
        List<PathNode> nodes = List.of(new PathNode(nid, 100, 100, 0), new PathNode(a, 70, 100, 0), new PathNode(b, 130, 100, 0), new PathNode(c, 100, 130, 0));
        List<PathEdge> edges = List.of(
            new PathEdge(new UUID(2, 1), nid, a, List.of(), "default"),
            new PathEdge(new UUID(2, 2), nid, b, List.of(), "default"),
            new PathEdge(new UUID(2, 3), nid, c, List.of(), "default"));
        PathNetwork net = new PathNetwork(new UUID(3, 3), "Junction", 99L, nodes, edges, List.of(prof));
        dump("junction", planner.plan(net, def, open));
    }
}

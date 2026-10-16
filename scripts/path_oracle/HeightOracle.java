import java.util.*;
import com.rspsi.tools.PathOverlayFitter;
/** Runs the original, extracted SceneGraph.smoothPathHeights with inert UI/history adapters. */
public class HeightOracle {
    public static void main(String[] args) {
        for (int c = 0; c < 16; c++) {
            int width = 16, length = 13, plane = c % 4;
            SceneGraph scene = new SceneGraph(width, length);
            for (int z = 0; z < 4; z++) for (int x = 0; x < width; x++) for (int y = 0; y < length; y++)
                scene.region.tileHeights[z][x][y] = -40 - (x * 19 + y * 31) % 110 - z * 160;
            Options.pathWidth.value = c % 2 == 0 ? 3 : 2.5;
            Options.pathHeightSmoothing.value = c < 4 ? 1 : c < 12 ? 5 : 250;
            Options.pathHeightBlend.value = c < 8 ? 0 : c < 12 ? 4 : 30;
            List<double[]> points = new ArrayList<>();
            points.add(new double[]{c % 2 == 0 ? 0.5 : 3.5, 1.5});
            points.add(new double[]{9.5, 10.5});
            points.add(new double[]{15.5, 2.5});
            List<double[]> line = PathOverlayFitter.smooth(points);
            System.out.println("case " + c + " " + width + " " + length + " " + plane + " " + Options.pathWidth.value + " " + (int) Options.pathHeightSmoothing.value + " " + Options.pathHeightBlend.value);
            for (double[] p : line) System.out.println("point " + p[0] + " " + p[1]);
            scene.smoothPathHeights(line, plane);
            for (int z = 0; z < 4; z++) for (int x = 0; x < width; x++) for (int y = 0; y < length; y++)
                System.out.println("height " + z + " " + x + " " + y + " " + scene.region.tileHeights[z][x][y]);
            System.out.println("end");
        }
    }
}

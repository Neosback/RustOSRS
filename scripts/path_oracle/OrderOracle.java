import java.util.*;
/** Integer map traversal, including deliberately treeified bins and resize splits. */
public class OrderOracle {
    public static void main(String[] args) {
        for (int kind = 0; kind < 6; kind++) {
            Map<Integer, Integer> map = new HashMap<>();
            List<Integer> keys = new ArrayList<>();
            int count = new int[]{12,13,96,600,1000,2000}[kind];
            for (int i = 0; i < count; i++) {
                int x = kind < 2 ? i : kind == 2 ? i * 64 : i * 29 % 32768;
                int y = kind < 2 ? 0 : kind == 2 ? 0 : kind == 3 ? x : i * 37 % 32768;
                keys.add(x << 16 | y);
            }
            // Treeify one bin first, then trigger several splits with unrelated keys.
            if (kind == 5) for (int i = 0; i < 100; i++) keys.add(i << 16 | i);
            System.out.println("case " + kind);
            for (int key : keys) { if (!map.containsKey(key)) { System.out.println("key " + Integer.toUnsignedString(key)); map.put(key, key); } }
            for (int key : map.keySet()) System.out.println("out " + Integer.toUnsignedString(key));
            System.out.println("end");
        }
    }
}

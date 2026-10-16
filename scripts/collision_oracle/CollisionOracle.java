import java.security.MessageDigest;
import java.util.HexFormat;

/** Exercises the pinned, unmodified deob CollisionMap; no collision implementation lives here. */
public final class CollisionOracle {
    private static void emit(String kind, boolean loaded, int type, int angle,
                             boolean projectile, int x, int y, int width, int height) throws Exception {
        CollisionMap map = new CollisionMap(104, 104, false);
        if (loaded) {
            for (int sx = 1; sx < 99; sx++) {
                for (int sy = 1; sy < 99; sy++) {
                    map.method6355(sx, sy, 1073741824);
                }
            }
        }
        switch (kind) {
            case "wall" -> map.method6340(x, y, type, angle, projectile);
            case "object" -> map.addGameObject(x, y, width, height, projectile);
            case "floor" -> map.setBlockedByFloor(x, y);
            case "decor" -> map.setBlockedByFloorDec(x, y);
            case "empty" -> {}
            default -> throw new IllegalArgumentException(kind);
        }
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        for (int sx = 0; sx < 104; sx++) {
            for (int sy = 0; sy < 104; sy++) {
                int value = map.flags[sx][sy];
                digest.update((byte) (value >>> 24));
                digest.update((byte) (value >>> 16));
                digest.update((byte) (value >>> 8));
                digest.update((byte) value);
            }
        }
        System.out.printf("%s %d %d %d %d %d %d %d %d %s%n", kind, loaded ? 1 : 0,
            type, angle, projectile ? 1 : 0, x, y, width, height,
            HexFormat.of().formatHex(digest.digest()));
    }

    public static void main(String[] args) throws Exception {
        System.out.println("# kind loaded type angle projectile x y width height sha256(x-major big-endian u32 flags)");
        emit("empty", false, 0, 0, false, 32, 32, 0, 0);
        emit("empty", true, 0, 0, false, 32, 32, 0, 0);
        for (boolean loaded : new boolean[]{false, true}) {
            for (boolean projectile : new boolean[]{false, true}) {
                for (int type = 0; type < 4; type++) {
                    for (int angle = 0; angle < 4; angle++) {
                        emit("wall", loaded, type, angle, projectile, 32, 32, 0, 0);
                    }
                }
                for (int[] size : new int[][]{{1, 1}, {2, 3}, {0, 0}, {100, 100}, {65535, 1}}) {
                    emit("object", loaded, 10, 0, projectile, 32, 32, size[0], size[1]);
                }
                emit("object", loaded, 9, 0, projectile, 102, 102, 4, 3);
            }
            emit("floor", loaded, 0, 0, false, 32, 32, 0, 0);
            emit("decor", loaded, 22, 0, false, 32, 32, 0, 0);
        }
    }
}

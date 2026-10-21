import java.util.Scanner;

/** Input/output adapter only; all tested normal arithmetic executes extracted pinned methods. */
public class NormalOracle {
    static ModelData model(Scanner input) {
        ModelData model = new ModelData();
        model.verticesCount = input.nextInt();
        model.faceCount = input.nextInt();
        model.verticesX = new int[model.verticesCount];
        model.verticesY = new int[model.verticesCount];
        model.verticesZ = new int[model.verticesCount];
        for (int i = 0; i < model.verticesCount; i++) {
            model.verticesX[i] = input.nextInt();
            model.verticesY[i] = input.nextInt();
            model.verticesZ[i] = input.nextInt();
        }
        model.indices1 = new int[model.faceCount];
        model.indices2 = new int[model.faceCount];
        model.indices3 = new int[model.faceCount];
        for (int i = 0; i < model.faceCount; i++) {
            model.indices1[i] = input.nextInt();
            model.indices2[i] = input.nextInt();
            model.indices3[i] = input.nextInt();
        }
        if (input.nextInt() != 0) {
            model.faceRenderTypes = new byte[model.faceCount];
            for (int i = 0; i < model.faceCount; i++) {
                model.faceRenderTypes[i] = input.nextByte();
            }
        }
        return model;
    }

    static String normals(VertexNormal[] normals) {
        if (normals == null) return "null";
        StringBuilder result = new StringBuilder("[");
        for (int i = 0; i < normals.length; i++) {
            if (i > 0) result.append(',');
            VertexNormal normal = normals[i];
            if (normal == null) result.append("null");
            else result.append("{\"x\":").append(normal.x)
                .append(",\"y\":").append(normal.y).append(",\"z\":").append(normal.z)
                .append(",\"magnitude\":").append(normal.magnitude).append('}');
        }
        return result.append(']').toString();
    }

    static String faceNormals(FaceNormal[] normals) {
        if (normals == null) return "null";
        StringBuilder result = new StringBuilder("[");
        for (int i = 0; i < normals.length; i++) {
            if (i > 0) result.append(',');
            FaceNormal normal = normals[i];
            if (normal == null) result.append("null");
            else result.append("{\"x\":").append(normal.x)
                .append(",\"y\":").append(normal.y).append(",\"z\":").append(normal.z).append('}');
        }
        return result.append(']').toString();
    }

    static String types(byte[] types) {
        if (types == null) return "null";
        StringBuilder result = new StringBuilder("[");
        for (int i = 0; i < types.length; i++) {
            if (i > 0) result.append(',');
            result.append(Byte.toUnsignedInt(types[i]));
        }
        return result.append(']').toString();
    }

    public static void main(String[] args) {
        Scanner input = new Scanner(System.in);
        int kind = input.nextInt();
        ModelData left = model(input);
        String result;
        if (kind == 0) {
            left.calculateVertexNormals();
            result = "\"kind\":\"base_normals\",\"vertex_normals\":" + normals(left.vertexNormals)
                + ",\"face_normals\":" + faceNormals(left.faceNormals);
        } else {
            ModelData right = model(input);
            int x = input.nextInt(), y = input.nextInt(), z = input.nextInt();
            ModelData.method5262(left, right, x, y, z, input.nextInt() != 0);
            result = "\"kind\":\"normal_merge\",\"left_merged_vertex_normals\":" + normals(left.vertexVertices)
                + ",\"right_merged_vertex_normals\":" + normals(right.vertexVertices)
                + ",\"left_face_render_types\":" + types(left.faceRenderTypes)
                + ",\"right_face_render_types\":" + types(right.faceRenderTypes);
        }
        if (input.hasNext()) throw new IllegalArgumentException("Unexpected trailing input");
        System.out.println("{\"schema_version\":1,\"case\":{" + result + "}}");
    }
}

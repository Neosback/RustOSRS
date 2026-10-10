import java.nio.file.Paths;

/** Probe: can the pinned deob's ObjectComposition.decode read every build-241 object definition? */
public class ObjectDecodeProbe {
	public static void main(String[] args) throws Exception {
		CacheArchives cache = new CacheArchives(Paths.get(args[0]));
		CacheArchives.FileArchive configs = new CacheArchives.FileArchive(cache, 2, false, true);
		int ok = 0, failed = 0, trailing = 0, empty = 0;
		int maxId = configs.fileIds[6] == null ? -1 : configs.files[6].length;
		System.out.println("objects group files=" + maxId);
		java.util.TreeMap<String, Integer> errors = new java.util.TreeMap<>();
		for (int id = 0; id < maxId; ++id) {
			byte[] bytes = configs.takeFile(6, id);
			if (bytes == null) {
				++empty;
				continue;
			}
			ObjectComposition def = new ObjectComposition();
			def.id = id;
			Buffer buf = new Buffer(bytes);
			try {
				def.decode(buf);
				def.postDecode();
				if (buf.offset * 2108391709 != bytes.length) {
					++trailing;
				}
				++ok;
			} catch (Throwable t) {
				++failed;
				errors.merge(t.getClass().getSimpleName() + ":" + String.valueOf(t.getMessage()).replaceAll("\\d+", "N"), 1, Integer::sum);
			}
		}
		System.out.println("ok=" + ok + " failed=" + failed + " trailing=" + trailing + " empty=" + empty);
		errors.forEach((k, v) -> System.out.println(v + " x " + k));
	}
}

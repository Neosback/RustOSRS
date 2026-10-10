import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.file.Path;

/**
 * Backs the deob client's {@code AbstractArchive} with a disk cache (main_file_cache.dat2/idxN)
 * so the pinned client's own loaders (object/floor definitions, ModelData) read real data.
 *
 * Default package so package-private deob members are reachable.
 */
public class CacheArchives {
	static final int SECTOR = 520;
	final RandomAccessFile dat;
	final Path dir;

	public CacheArchives(Path dir) throws IOException {
		this.dir = dir;
		this.dat = new RandomAccessFile(dir.resolve("main_file_cache.dat2").toFile(), "r");
	}

	/** Container bytes of {@code group} in {@code index} (255 = reference tables); null if absent. */
	byte[] readContainer(int index, int group) throws IOException {
		try (RandomAccessFile idx = new RandomAccessFile(dir.resolve("main_file_cache.idx" + index).toFile(), "r")) {
			if ((long) (group + 1) * 6 > idx.length()) {
				return null;
			}
			idx.seek((long) group * 6);
			byte[] e = new byte[6];
			idx.readFully(e);
			int length = (e[0] & 255) << 16 | (e[1] & 255) << 8 | (e[2] & 255);
			int sector = (e[3] & 255) << 16 | (e[4] & 255) << 8 | (e[5] & 255);
			if (length <= 0 || sector <= 0) {
				return null;
			}
			byte[] out = new byte[length];
			int read = 0;
			int part = 0;
			byte[] buf = new byte[SECTOR];
			while (read < length) {
				dat.seek((long) sector * SECTOR);
				boolean big = group > 65535;
				int header = big ? 10 : 8;
				int chunk = Math.min(length - read, SECTOR - header);
				dat.readFully(buf, 0, header + chunk);
				int g = big
					? (buf[0] & 255) << 24 | (buf[1] & 255) << 16 | (buf[2] & 255) << 8 | (buf[3] & 255)
					: (buf[0] & 255) << 8 | (buf[1] & 255);
				int o = big ? 4 : 2;
				int p = (buf[o] & 255) << 8 | (buf[o + 1] & 255);
				int next = (buf[o + 2] & 255) << 16 | (buf[o + 3] & 255) << 8 | (buf[o + 4] & 255);
				int ix = buf[o + 5] & 255;
				if (g != group || p != part || ix != index) {
					throw new IOException("sector chain mismatch idx=" + index + " group=" + group
						+ " got g=" + g + " part=" + p + " idx=" + ix);
				}
				System.arraycopy(buf, header, out, read, chunk);
				read += chunk;
				++part;
				sector = next;
			}
			return out;
		}
	}

	/** One logical archive (content index) exposed through the client's AbstractArchive API. */
	public static class FileArchive extends AbstractArchive {
		final CacheArchives owner;
		final int index;

		public FileArchive(CacheArchives owner, int index, boolean releaseGroups, boolean shallowFiles) throws IOException {
			super(releaseGroups, shallowFiles);
			this.owner = owner;
			this.index = index;
			byte[] ref = owner.readContainer(255, index);
			if (ref == null) {
				throw new IOException("no reference table for index " + index);
			}
			this.decodeIndex(ref);
		}

		@Override
		void loadGroup(int group) {
			try {
				byte[] container = owner.readContainer(index, group);
				if (container != null) {
					this.groups[group] = container;
				}
			} catch (IOException e) {
				throw new RuntimeException(e);
			}
		}
	}
}

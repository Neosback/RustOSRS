import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.SequenceInputStream;
import org.apache.commons.compress.compressors.bzip2.BZip2CompressorInputStream;

/**
 * Harness overlay for the deob's {@code BZip2Decompressor}.
 *
 * The pinned deob snapshot's hand-rolled decompressor carries overflowed block-size constants and
 * cannot run. The harness shadows it (first on the source path) with a standard bzip2 decoder.
 * The cache stores streams without the leading {@code BZh9} header; this restores it.
 */
public final class BZip2Decompressor {
	public static int BZip2Decompressor_decompress(byte[] dest, int destLength, byte[] src, int srcLength, int srcStart) {
		try (BZip2CompressorInputStream in = new BZip2CompressorInputStream(new SequenceInputStream(
			new ByteArrayInputStream(new byte[]{'B', 'Z', 'h', '9'}),
			new ByteArrayInputStream(src, srcStart, src.length - srcStart)))) {
			int total = 0;
			while (total < destLength) {
				int n = in.read(dest, total, destLength - total);
				if (n < 0) {
					break;
				}
				total += n;
			}
			return destLength - total;
		} catch (IOException e) {
			throw new RuntimeException(e);
		}
	}
}

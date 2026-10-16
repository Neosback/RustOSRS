package net.runelite.rs.api;

import java.io.FileDescriptor;
import net.runelite.mapping.Import;

public interface RSBufferedNetSocket extends RSAbstractSocket
{
	@Import("source")
	RSBufferedSource getSource();

	FileDescriptor getFileDescriptor();
}

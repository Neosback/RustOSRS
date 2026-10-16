package net.runelite.rs.api;

import java.io.InputStream;
import net.runelite.mapping.Import;

public interface RSBufferedSource
{
	@Import("inputStream")
	InputStream getInputStream();
}

package net.runelite.rs.api;

import net.runelite.mapping.Import;

public interface RSPacketWriter
{
	@Import("socket")
	RSAbstractSocket getSocket();
}
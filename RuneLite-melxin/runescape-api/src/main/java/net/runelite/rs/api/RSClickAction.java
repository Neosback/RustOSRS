package net.runelite.rs.api;

import net.runelite.mapping.Import;

public interface RSClickAction
{
	@Import("rsOrdinal")
	int getAction();
}

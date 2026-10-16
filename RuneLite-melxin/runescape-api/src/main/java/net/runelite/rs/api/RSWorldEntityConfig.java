package net.runelite.rs.api;

import net.runelite.api.WorldEntityConfig;
import net.runelite.mapping.Import;

public interface RSWorldEntityConfig extends WorldEntityConfig
{
	@Import("id")
	@Override
	int getId();

	@Import("boundsX")
	@Override
	int getBoundsX();

	@Import("boundsY")
	@Override
	int getBoundsY();

	@Import("boundsWidth")
	@Override
	int getBoundsWidth();

	@Import("boundsHeight")
	@Override
	int getBoundsHeight();
}

package net.runelite.rs.api;

import net.runelite.mapping.Import;

public interface RSPendingSpawn
{
	@Import("plane")
	int getPlane();

	@Import("type")
	int getType();

	@Import("x")
	int getX();

	@Import("y")
	int getY();

	@Import("objectId")
	int getObjectId();

	@Import("objectId2")
	int getObjectId2();

	@Import("delay")
	int getDelay();

	@Import("hitpoints")
	int getHitPoints();

	@Import("getOpOverride")
	String getOpOverride(int index);

	@Import("isOpShown")
	boolean isOpShown(int index);
}
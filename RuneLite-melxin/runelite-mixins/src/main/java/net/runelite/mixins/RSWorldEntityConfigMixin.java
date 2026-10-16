package net.runelite.mixins;

import net.runelite.api.mixins.Inject;
import net.runelite.api.mixins.MethodHook;
import net.runelite.api.mixins.Mixin;
import net.runelite.rs.api.RSBuffer;
import net.runelite.rs.api.RSWorldEntityConfig;

@Mixin(RSWorldEntityConfig.class)
public abstract class RSWorldEntityConfigMixin implements RSWorldEntityConfig
{
	@Inject
	private int category;

	@MethodHook("decodeNext")
	@Inject
	public void onDecodeNext(RSBuffer buffer, int opcode)
	{
		if (opcode == 20)
		{
			int offset = buffer.getOffset();
			this.category = buffer.readUnsignedShort();
			buffer.setOffset(offset);
		}
	}

	@Override
	@Inject
	public int getCategory()
	{
		return this.category;
	}
}

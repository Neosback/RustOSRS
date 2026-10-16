package net.runelite.mixins;

import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.IOException;
import javax.annotation.Nullable;
import net.runelite.api.mixins.Inject;
import net.runelite.api.mixins.Mixin;
import net.runelite.rs.api.RSBufferedNetSocket;

@Mixin(RSBufferedNetSocket.class)
public abstract class RSBufferedNetSocketMixin implements RSBufferedNetSocket
{
	@Nullable
	@Inject
	@Override
	public FileDescriptor getFileDescriptor()
	{
		final FileInputStream fis = (FileInputStream) this.getSource().getInputStream();
		try
		{
			return fis.getFD();
		}
		catch (IOException e)
		{
			return null;
		}
	}
}

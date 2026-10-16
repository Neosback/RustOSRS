import net.runelite.mapping.Export;
import net.runelite.mapping.Implements;
import net.runelite.mapping.ObfuscatedGetter;
import net.runelite.mapping.ObfuscatedName;
import net.runelite.mapping.ObfuscatedSignature;

@ObfuscatedName("pw")
@Implements("WorldEntityOwnerType")
public enum WorldEntityOwnerType implements Enum {
	@ObfuscatedName("av")
	@ObfuscatedSignature(
		descriptor = "Lpw;"
	)
	field4844(0, 0),
	@ObfuscatedName("at")
	@ObfuscatedSignature(
		descriptor = "Lpw;"
	)
	field4843(1, 1),
	@ObfuscatedName("ag")
	@ObfuscatedSignature(
		descriptor = "Lpw;"
	)
	field4842(2, 2);

	@ObfuscatedName("an")
	@ObfuscatedGetter(
		intValue = 624376631
	)
	final int field4845;
	@ObfuscatedName("ae")
	@ObfuscatedGetter(
		intValue = 562405807
	)
	final int field4846;

	WorldEntityOwnerType(int var3, int var4) {
		this.field4845 = var3;
		this.field4846 = var4;
	}

	@ObfuscatedName("av")
	@ObfuscatedSignature(
		descriptor = "(B)I",
		garbageValue = "20"
	)
	@Export("rsOrdinal")
	public int rsOrdinal() {
		return this.field4846;
	}
}

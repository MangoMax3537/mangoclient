package gg.mangoclient.mangoconfig.mixin;

import net.minecraft.client.option.SimpleOption;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

@Mixin(SimpleOption.class)
public interface SimpleOptionAccessor {
	@Accessor("value")
	Object mangoconfig$getRawValue();

	@Accessor("value")
	void mangoconfig$setRawValue(Object value);
}

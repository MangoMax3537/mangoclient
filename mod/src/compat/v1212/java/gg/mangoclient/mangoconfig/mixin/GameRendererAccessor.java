package gg.mangoclient.mangoconfig.mixin;

import net.minecraft.client.render.GameRenderer;
import net.minecraft.util.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** Vanilla's own way into its post effect slot, for Colour Saturation. */
@Mixin(GameRenderer.class)
public interface GameRendererAccessor {
	@Invoker("setPostProcessor")
	void mangoconfig$setPostProcessor(Identifier id);
}

package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.WorldRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Block Overlay, 1.21.2+: drawBlockOutline takes the outline's ARGB colour as
 * its only int, whatever else its arguments became over the versions, so the
 * colour is swapped on the way in. High contrast mode calls it twice and gets
 * the same colour both times.
 */
@Mixin(WorldRenderer.class)
public class BlockOutlineMixin {
	@ModifyVariable(method = "drawBlockOutline", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private int mangoconfig$outline(int colour) {
		return Visuals.outline(colour);
	}
}

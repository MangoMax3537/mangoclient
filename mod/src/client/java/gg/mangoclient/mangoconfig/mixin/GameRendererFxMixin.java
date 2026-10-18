package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Colour saturation is settled once a frame, before the world is drawn. The
 * frame's arguments changed in 1.21 and none of them are needed here, so the
 * handler takes none.
 */
@Mixin(GameRenderer.class)
public class GameRendererFxMixin {
	@Inject(method = "render", at = @At("HEAD"))
	private void mangoconfig$saturation(CallbackInfo ci) {
		Visuals.tickSaturation((GameRenderer) (Object) this);
	}
}

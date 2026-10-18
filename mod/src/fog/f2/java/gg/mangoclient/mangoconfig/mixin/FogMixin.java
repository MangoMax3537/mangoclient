package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.BackgroundRenderer;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.Fog;
import org.joml.Vector4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Custom Fog, 1.21.2-1.21.5: the terrain fog comes back from applyFog as a
 * record, so the one returned is swapped for a stretched copy. The sky keeps
 * its own fog; pushing that away leaves a hard horizon.
 */
@Mixin(BackgroundRenderer.class)
public class FogMixin {
	@Inject(method = "applyFog", at = @At("RETURN"), cancellable = true)
	private static void mangoconfig$fog(Camera camera, BackgroundRenderer.FogType fogType, Vector4f color,
			float viewDistance, boolean thickenFog, float tickDelta, CallbackInfoReturnable<Fog> cir) {
		if (fogType != BackgroundRenderer.FogType.FOG_TERRAIN) return;
		float scale = Visuals.fogScale();
		if (scale == 1f) return;
		Fog fog = cir.getReturnValue();
		cir.setReturnValue(new Fog(fog.start() * scale, fog.end() * scale, fog.shape(),
			fog.red(), fog.green(), fog.blue(), fog.alpha()));
	}
}

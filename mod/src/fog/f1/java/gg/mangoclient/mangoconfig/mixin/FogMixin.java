package gg.mangoclient.mangoconfig.mixin;

import com.mojang.blaze3d.systems.RenderSystem;
import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.BackgroundRenderer;
import net.minecraft.client.render.Camera;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Custom Fog, 1.20.5-1.21.1: vanilla works out the terrain fog and hands it
 * straight to RenderSystem, so it is stretched there once vanilla is done.
 * The sky keeps its own fog; pushing that away leaves a hard horizon.
 */
@Mixin(BackgroundRenderer.class)
public class FogMixin {
	@Inject(method = "applyFog", at = @At("TAIL"))
	private static void mangoconfig$fog(Camera camera, BackgroundRenderer.FogType fogType, float viewDistance,
			boolean thickFog, float tickDelta, CallbackInfo ci) {
		if (fogType != BackgroundRenderer.FogType.FOG_TERRAIN) return;
		float scale = Visuals.fogScale();
		if (scale == 1f) return;
		RenderSystem.setShaderFogStart(RenderSystem.getShaderFogStart() * scale);
		RenderSystem.setShaderFogEnd(RenderSystem.getShaderFogEnd() * scale);
	}
}

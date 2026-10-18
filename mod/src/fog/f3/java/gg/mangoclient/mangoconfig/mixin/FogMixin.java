package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.fog.FogRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Custom Fog, 1.21.6+: every fog the world uses is written into one uniform
 * buffer by this private method, as environmental start and end followed by
 * render-distance start and end (then the sky and clouds, left alone so the
 * horizon keeps its blend). Stretching the first four on the way in moves the
 * fog without caring which of vanilla's modifiers produced it.
 */
@Mixin(FogRenderer.class)
public class FogMixin {

	@ModifyVariable(method = "applyFog(Ljava/nio/ByteBuffer;ILorg/joml/Vector4f;FFFFFF)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float mangoconfig$environmentalStart(float value) {
		return value * Visuals.fogScale();
	}

	@ModifyVariable(method = "applyFog(Ljava/nio/ByteBuffer;ILorg/joml/Vector4f;FFFFFF)V", at = @At("HEAD"), argsOnly = true, ordinal = 1)
	private float mangoconfig$environmentalEnd(float value) {
		return value * Visuals.fogScale();
	}

	@ModifyVariable(method = "applyFog(Ljava/nio/ByteBuffer;ILorg/joml/Vector4f;FFFFFF)V", at = @At("HEAD"), argsOnly = true, ordinal = 2)
	private float mangoconfig$renderDistanceStart(float value) {
		return value * Visuals.fogScale();
	}

	@ModifyVariable(method = "applyFog(Ljava/nio/ByteBuffer;ILorg/joml/Vector4f;FFFFFF)V", at = @At("HEAD"), argsOnly = true, ordinal = 3)
	private float mangoconfig$renderDistanceEnd(float value) {
		return value * Visuals.fogScale();
	}
}

package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.SimpleOption;
import net.minecraft.client.render.LightmapTextureManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Fullbright and Brightness: the raw gamma is swapped in only while vanilla
 * builds the lightmap, so the slider in its own options never moves.
 */
@Mixin(LightmapTextureManager.class)
public class LightmapTextureManagerMixin {
	@Unique private Object mangoconfig$normalGamma;
	@Unique private boolean mangoconfig$gammaInjected;

	@Inject(method = "update", at = @At("HEAD"))
	private void mangoconfig$injectFullbright(float delta, CallbackInfo ci) {
		Double override = Visuals.gammaOverride();
		if (override == null) return;
		SimpleOption<Double> gamma = MinecraftClient.getInstance().options.getGamma();
		SimpleOptionAccessor raw = (SimpleOptionAccessor) (Object) gamma;
		mangoconfig$normalGamma = raw.mangoconfig$getRawValue();
		raw.mangoconfig$setRawValue(override);
		mangoconfig$gammaInjected = true;
	}

	@Inject(method = "update", at = @At("RETURN"))
	private void mangoconfig$restoreBrightness(float delta, CallbackInfo ci) {
		if (!mangoconfig$gammaInjected) return;
		SimpleOption<Double> gamma = MinecraftClient.getInstance().options.getGamma();
		((SimpleOptionAccessor) (Object) gamma).mangoconfig$setRawValue(mangoconfig$normalGamma);
		mangoconfig$normalGamma = null;
		mangoconfig$gammaInjected = false;
	}
}

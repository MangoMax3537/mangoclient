package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.GameSettings;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.SimpleOption;
import net.minecraft.client.render.LightmapTextureManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Temporarily exposes raw gamma 1500.0 only while vanilla builds the lightmap. */
@Mixin(LightmapTextureManager.class)
public class LightmapTextureManagerMixin {
	@Unique private Object mangoconfig$normalGamma;
	@Unique private boolean mangoconfig$gammaInjected;

	@Inject(method = "update", at = @At("HEAD"))
	private void mangoconfig$injectFullbright(float delta, CallbackInfo ci) {
		if (!GameSettings.fullbrightEnabled()) return;
		SimpleOption<Double> gamma = MinecraftClient.getInstance().options.getGamma();
		SimpleOptionAccessor raw = (SimpleOptionAccessor) (Object) gamma;
		mangoconfig$normalGamma = raw.mangoconfig$getRawValue();
		raw.mangoconfig$setRawValue(1500.0D);
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

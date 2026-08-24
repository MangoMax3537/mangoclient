package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.MangoConfig;
import gg.mangoclient.mangoconfig.Mods;
import net.minecraft.client.Mouse;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Uses the wheel for zoom strength while zoom is being held. */
@Mixin(Mouse.class)
public abstract class MouseMixin {
	@Inject(method = "onMouseScroll", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$zoomScroll(long window, double horizontal, double vertical, CallbackInfo ci) {
		if (!Mods.zoomActive) return;

		// Vanilla falls back to horizontal scrolling when a device supplies no
		// vertical delta. Mirror that behaviour so touchpads work as expected.
		double amount = vertical != 0.0 ? vertical : -horizontal;
		if (amount == 0.0) return;

		int before = Mods.zoomLevel.get();
		Mods.zoomLevel.set(before + (amount > 0.0 ? 1 : -1));
		if (Mods.zoomLevel.get() != before) MangoConfig.save();

		// Do not let the same wheel event reach spectator speed or the hotbar.
		ci.cancel();
	}
}

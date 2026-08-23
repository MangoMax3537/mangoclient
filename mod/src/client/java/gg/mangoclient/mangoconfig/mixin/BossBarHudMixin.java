package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Mods;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.BossBarHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Hide Boss Bar, hooked at the bar's own renderer rather than the HUD: this
 * class has drawn the bars with the same signature since 1.20, so one hook
 * serves every game version the mod builds for.
 */
@Mixin(BossBarHud.class)
public class BossBarHudMixin {
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void mangoconfig$hide(DrawContext context, CallbackInfo ci) {
		if (Mods.hideBossBar.value) ci.cancel();
	}
}

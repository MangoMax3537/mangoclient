package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.MangoConfig;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.client.render.RenderTickCounter;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The one hook MangoConfig needs: draw on top of the vanilla HUD once the game
 * has finished drawing its own. The keybind and the click counter are polled
 * from here too, so there is nothing else to attach to - which is what lets
 * the mod run without Fabric API.
 */
@Mixin(InGameHud.class)
public class GuiMixin {
	@Inject(method = "render", at = @At("TAIL"))
	private void mangoconfig$afterHud(DrawContext context, RenderTickCounter tickCounter, CallbackInfo ci) {
		MangoConfig.onHudRender(context);
	}
}

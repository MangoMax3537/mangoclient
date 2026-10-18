package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.MangoConfig;
import gg.mangoclient.mangoconfig.MangoTitleScreen;
import gg.mangoclient.mangoconfig.Ping;
import gg.mangoclient.mangoconfig.RPCManager;
import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.world.ClientWorld;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The one client tick the mod needs, the GUI scale an inventory gets, and
 * the home screen that replaces vanilla's title screen.
 *
 * The HUD hook cannot carry the tick: it only fires with a world on screen,
 * and sitting in the main menu is a state Discord should show as much as any
 * other. The client tick runs either way, and all this does with it is hand
 * over the client - the work of deciding and sending is not done here.
 */
@Mixin(MinecraftClient.class)
public class MinecraftClientMixin {
	@Shadow public Screen currentScreen;
	@Shadow public ClientWorld world;

	@Inject(method = "tick", at = @At("HEAD"))
	private void mangoconfig$clientTick(CallbackInfo ci) {
		MinecraftClient mc = (MinecraftClient) (Object) this;
		RPCManager.onClientTick(mc);
		Ping.onClientTick(mc);
		MangoConfig.audit(mc);
	}

	// A window resize recomputes the scale from the setting; an open
	// inventory asks for its own instead.
	@ModifyArg(method = "onResolutionChanged",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/Window;calculateScaleFactor(IZ)I"),
		index = 0)
	private int mangoconfig$inventoryScale(int guiScale) {
		return Visuals.guiScaleFor(currentScreen, guiScale);
	}

	// MangoClient's own home screen, every time vanilla would show its title
	// screen - including a null screen outside a world, which vanilla turns into
	// a title screen further down this same method.
	@ModifyVariable(method = "setScreen", at = @At("HEAD"), argsOnly = true)
	private Screen mangoconfig$homeScreen(Screen screen) {
		if (screen instanceof TitleScreen || (screen == null && world == null)) return new MangoTitleScreen();
		return screen;
	}

	@Inject(method = "setScreen", at = @At("RETURN"))
	private void mangoconfig$screenScale(Screen screen, CallbackInfo ci) {
		Visuals.applyGuiScale((MinecraftClient) (Object) this);
	}
}

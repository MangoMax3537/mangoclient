package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.client.network.ServerInfo;

import java.util.ArrayList;
import java.util.List;

/** Which server this is, and how busy it is. */
public class ServerModule extends TextModule {
	private final Option.Choice shows =
		new Option.Choice("shows", "Show", 0, "Address", "Name");
	private final Option.Bool players = new Option.Bool("players", "Player count", true);

	public ServerModule() {
		super("server", "Server", "The server you are on.", false, 0.01f, 0.22f);
		options.add(shows);
		options.add(players);
	}

	private String where(MinecraftClient mc, boolean preview) {
		ServerInfo info = mc.getCurrentServerEntry();
		if (info == null) return preview ? "play.example.net" : "Singleplayer";
		String name = shows.is("Name") && info.name != null && !info.name.isBlank() ? info.name : info.address;
		return name == null || name.isBlank() ? "Server" : name;
	}

	private int online(MinecraftClient mc) {
		ClientPlayNetworkHandler handler = mc.getNetworkHandler();
		return handler == null ? 0 : handler.getPlayerList().size();
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		out.add(where(mc, preview));
		if (players.value) {
			int count = online(mc);
			out.add(count + (count == 1 ? " player" : " players"));
		}
		return out;
	}
}

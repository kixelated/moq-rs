import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Broadcast } from "./broadcast";
import { Abortable, Context } from "../util/context";

export function onceler<Target extends (context: Context, ...args: any) => Promise<any>>(
	target: Target,
	context: ClassMethodDecoratorContext<ThisParameterType<Target>, Target>,
) {
	let active: Context | undefined = undefined;

	return (...args: Parameters<Target>) => {
		active?.abort();
		active = new Context();

		return target(active, ...args);
	};
}

// This class must be created on the main thread due to AudioContext.
export class Watch {
	#url?: string;
	#canvas?: HTMLCanvasElement;

	#connection?: Abortable<Moq.Connection>;
	#discover?: Abortable<void>;
	#catalog?: Abortable<Catalog.Broadcast>;
	#running?: Abortable<void>;

	@onceler
	async #connectTest(context: Context, url: string): Promise<Moq.Connection> {
		return Moq.Connection.connect(url);
	}

	get url(): string | undefined {
		this.#connectTest(url);
		return this.#url;
	}

	set url(url: string | undefined) {
		this.#url = url;
		this.#connection?.abort();

		if (url) {
			this.#connection = new Abortable((context) => this.#connect(context, url));
		} else {
			this.#connection = undefined;
		}
	}

	async #connect(context: Context, url: string): Promise<Moq.Connection> {
		// TODO support abort
		const connection = await Moq.Connection.connect(url);
		context.done.finally(() => connection.close());

		if (!context.aborted) {
			this.#discover?.abort();
			this.#discover = new Abortable((context) => this.#runDiscover(context, connection));
		}

		return connection;
	}

	async #runDiscover(context: Context, connection: Moq.Connection) {
		// Remove the leading slash
		const path = connection.url.pathname.slice(1);
		const announced = await connection.announced(path);

		for (;;) {
			const announce = await Promise.any([announced.next(), context.done]);
			if (!announce) break;

			if (!announce.path.endsWith("catalog.json")) {
				continue;
			}

			console.log("found catalog", announce.path);

			this.#catalog?.abort();
			this.#catalog = new Abortable((context) => this.#runCatalog(context, connection, announce.path));
		}
	}

	async #runCatalog(context: Context, connection: Moq.Connection, path: string): Promise<Catalog.Broadcast> {
		const catalog = await Catalog.Broadcast.fetch(connection, path);
		if (!context.aborted) {
			this.#running = new Abortable((context) => this.#run(context, connection, catalog));
		}

		return catalog;
	}

	async #run(context: Context, connection: Moq.Connection, catalog: Catalog.Broadcast) {}
}

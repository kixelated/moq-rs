import assert from "node:assert";
import test from "node:test";
import { Track } from "./track";

test("track clone", async () => {
	const track = new Track("test", 0);
	const writer = track.writer;

	// Clone the reader before we append any groups
	const readerA = track.reader;
	const readerB = readerA.clone();

	const group1 = await writer.appendGroup();

	// Clone the reader after we appended that group; we still get it.
	const readerC = readerA.clone();

	const group1A = await readerA.nextGroup();
	const group1B = await readerB.nextGroup();
	const group1C = await readerC.nextGroup();

	assert.strictEqual(group1A?.id, group1.id);
	assert.strictEqual(group1B?.id, group1.id);
	assert.strictEqual(group1C?.id, group1.id);

	// Append a new group, everybody gets it
	const group2 = await writer.appendGroup();

	const group2A = await readerA.nextGroup();
	const group2B = await readerB.nextGroup();
	const group2C = await readerC.nextGroup();

	assert.strictEqual(group2A?.id, group2.id);
	assert.strictEqual(group2B?.id, group2.id);
	assert.strictEqual(group2C?.id, group2.id);

	// Clone the reader after we appended that group.
	// This new reader gets the most recent group but that's it.
	const readerD = readerA.clone();

	const group2D = await readerD.nextGroup();
	assert.strictEqual(group2D?.id, group2.id);

	// Everybody gets the new group
	const group3 = await writer.appendGroup();

	const group3A = await readerA.nextGroup();
	const group3B = await readerB.nextGroup();
	const group3C = await readerC.nextGroup();
	const group3D = await readerD.nextGroup();

	assert.strictEqual(group3A?.id, group3.id);
	assert.strictEqual(group3B?.id, group3.id);
	assert.strictEqual(group3C?.id, group3.id);
	assert.strictEqual(group3D?.id, group3.id);

	// It's okay to close readers.
	readerA.close();
	readerB.close();

	const group4 = await writer.appendGroup();

	const group4A = await readerA.nextGroup();
	const group4B = await readerB.nextGroup();
	const group4C = await readerC.nextGroup();
	const group4D = await readerD.nextGroup();

	assert.strictEqual(group4A?.id, undefined);
	assert.strictEqual(group4B?.id, undefined);
	assert.strictEqual(group4C?.id, group4.id);
	assert.strictEqual(group4D?.id, group4.id);

	const readerE = readerC.clone();
	const group4E = await readerE.nextGroup();
	assert.strictEqual(group4E?.id, group4.id);
});

test("track group cloning", async () => {
	const track = new Track("test", 0);
	const writer = track.writer;

	const readerA = track.reader;
	const readerB = readerA.clone();

	// Make sure both readers get separate copies of the groups.
	const group = await writer.appendGroup();
	await group.writeFrame(new Uint8Array([1]));
	await group.writeFrame(new Uint8Array([2]));
	await group.writeFrame(new Uint8Array([3]));

	const groupA = await readerA.nextGroup();
	const groupB = await readerB.nextGroup();

	assert.strictEqual(groupA?.id, group.id);
	assert.strictEqual(groupB?.id, group.id);

	const frame1A = await groupA.readFrame();
	const frame1B = await groupB.readFrame();

	assert.deepEqual(frame1A, new Uint8Array([1]));
	assert.deepEqual(frame1B, new Uint8Array([1]));

	const frame2A = await groupA.readFrame();
	groupA.close(); // closing doesn't impact the other reader
	const frame2B = await groupB.readFrame();

	assert.deepEqual(frame2A, new Uint8Array([2]));
	assert.deepEqual(frame2B, new Uint8Array([2]));

	const frame3A = await groupA.readFrame();
	const frame3B = await groupB.readFrame();

	assert.deepEqual(frame3A, undefined);
	assert.deepEqual(frame3B, new Uint8Array([3]));
});

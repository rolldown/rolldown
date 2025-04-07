import { PrismaClient } from '@prisma/client'

const prisma = new PrismaClient()


async function main() {
  const transformCalls = await prisma.pluginHookTransformCall.findMany({
    include: {
      pluginHookTransformStart: true,
      PluginHookTransformEnd: {
        select: {
          transformedSource: true,
        }
      },
    }
  });
  for (const transformCall of transformCalls) {
    console.log('processing data');
  }
}
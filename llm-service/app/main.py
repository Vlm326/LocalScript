import asyncio
from pipeline import GenerationPipeline 
from config import GENERATION_MODEL, ADDRESS, OLLAMA_PORT
from sandbox_client import send_code_for_validation, extract_validation_feedback



def main():
    GenerationPipline = GenerationPipeline(GENERATION_MODEL, ADDRESS, OLLAMA_PORT, max_retries = 2)

    #переменная для расчета времени работы моделей (в будущем убрать)
    total_time = 0

    user_prompt = input('Введите свой запрос: ')
    plan = GenerationPipeline._generate_plan(user_prompt, total_time)

    #блок iter1
    while user_answer != 'Подтвердить':
        print(f'''
            Код будет сгенерирован по следующему плану:
            {plan}
            Введите "Подтвердить" в случае, если план соответствует вашим ожиданиям. 
            В противном случае укажите, что следует убрать/добавить.
            ''')
        user_answer = input()
    
    '''
    ============================
    ПРОПУЩЕНО ЧАНКИРОВАНИЕ И RAG
    ============================
    '''

    #iter2
    def code_validation(plan, user_prompt, previous_code: str = '', feedback: str = ''):
        for i in range(5):
            code = GenerationPipeline._generate_code(plan, user_prompt, previous_code = previous_code, critic_feedback=feedback)
            validation_resp = asyncio.run(send_code_for_validation(code))
            extracted_feedback = extract_validation_feedback(validation_resp)
            if extracted_feedback is True:
                return code
            else:
                print(f'({i+1}) Найдена ошибка в коде, идет исправление...')
        else:
            raise RuntimeError(f"Не удалось сгенерировать рабочий код за {i+1} попыток")
        
    validated_code = code_validation(plan, user_prompt)
    while True:
        print(f'''
            {validated_code}
            Введите "Подтвердить" в случае, если код соответствует вашим ожиданиям.
            В противном случае укажите, что следует убрать/добавить''')
        user_answer = input()
        if user_answer == 'Подтвердить':
            return 1
        validated_code = code_validation(plan, user_prompt, previous_code=validated_code, feedback=user_answer)

    






    




    
if __name__ == "__main__":
    main()
